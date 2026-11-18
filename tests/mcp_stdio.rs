use base64::{Engine, engine::general_purpose::STANDARD};
use elmapa::{
    document::{Brush, Point},
    storage::{DocumentStore, LocalDocumentStore},
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

struct Client {
    process: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl Client {
    fn connect(root: &Path) -> Self {
        let mut process = Command::new(env!("CARGO_BIN_EXE_elmapa"))
            .args(["--mcp"])
            .arg(root)
            .env("ELMAPA_LIBRARY_DIR", root.join("isolated-library"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut client = Self {
            input: process.stdin.take(),
            output: BufReader::new(process.stdout.take().unwrap()),
            process,
            next_id: 1,
        };
        let info = client.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "elmapa-tests", "version": "1"}
            }),
        );
        assert_eq!(info["serverInfo"]["name"], "elmapa");
        client.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        client
    }

    fn send(&mut self, message: Value) {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{message}").unwrap();
        input.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let mut line = String::new();
        assert!(
            self.output.read_line(&mut line).unwrap() > 0,
            "server closed"
        );
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], id);
        assert!(response["error"].is_null(), "{response}");
        response["result"].clone()
    }

    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }

    fn text(&mut self, name: &str, arguments: Value) -> Value {
        let result = self.call(name, arguments);
        assert_ne!(result["isError"], true, "{result}");
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        drop(self.input.take());
        let _ = self.process.wait();
    }
}

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("elmapa-mcp-test-{:032x}", rand::random::<u128>())))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn codex_facing_stdio_creates_renders_edits_and_exports_editable_layers() {
    let root = Sandbox::new();
    let mut mcp = Client::connect(&root.0);
    let tools = mcp.request("tools/list", json!({}));
    let tools = tools["tools"].as_array().unwrap();
    for name in [
        "create_map",
        "inspect_map",
        "edit_map",
        "preview_map",
        "export_map",
    ] {
        assert!(tools.iter().any(|tool| tool["name"] == name), "{name}");
    }

    let catalog = mcp.text("inspect_map", json!({}));
    assert!(
        catalog["landmarks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "elmapa.basic:town")
    );
    let created = mcp.text("create_map", json!({
        "name": "Sable Island",
        "size": [640, 400],
        "features": [
            {"name":"Island", "art":{"kind":"area", "brush":"land",
                "outline":[[65,190],[132,87],[262,55],[454,96],[470,174],[363,208],[299,320],[155,320]]}},
            {"name":"Mooring", "art":{"kind":"landmark", "asset":"town", "at":[320,201], "size":60}},
            {"name":"Pass", "art":{"kind":"stroke", "brush":"path", "through":[[132,160],[205,116],"Mooring"]}},
            {"name":"Mooring title", "art":{"kind":"label", "text":"Mooring", "at":[330,235], "size":29}}
        ]
    }));
    let id = &created["map_id"];
    let path = Path::new(created["atlas_path"].as_str().unwrap());
    assert!(path.starts_with(root.0.canonicalize().unwrap()));
    let document = LocalDocumentStore.load(path).unwrap();
    assert_eq!(document.layers.len(), 4);
    assert!(document.layers[0].strokes.len() > 1);
    assert!(
        document.layers[0]
            .strokes
            .iter()
            .all(|stroke| stroke.creation_id.is_some())
    );
    assert_eq!(
        document.layers[0].strokes.iter().next().unwrap().brush,
        Brush::Land
    );
    assert_eq!(
        document.layers[2]
            .strokes
            .iter()
            .next()
            .unwrap()
            .points
            .last(),
        Some(&Point::new(320., 201.))
    );

    let preview = mcp.call("preview_map", json!({"map_id":id}));
    assert_ne!(preview["isError"], true);
    let image = &preview["content"][1];
    assert_eq!(image["type"], "image");
    assert_eq!(image["mimeType"], "image/png");
    let png = STANDARD.decode(image["data"].as_str().unwrap()).unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (512, 320));
    assert!(image.pixels().any(|pixel| pixel != image.get_pixel(0, 0)));

    // A second process can open the same atlas and uses the same revision.
    let mut editor = Client::connect(&root.0);
    let inspected = editor.text("inspect_map", json!({"map_id":id}));
    assert_eq!(inspected["revision"], created["revision"]);
    let mooring = &inspected["layers"][1]["id"];
    let pass = &inspected["layers"][2]["id"];
    let title = &inspected["layers"][3]["id"];
    let invalid = editor.call("edit_map", json!({
        "map_id": id, "expected_revision": created["revision"],
        "changes": [{"op":"move", "layer_id":mooring, "to":[299,225]}, {"op":"remove", "layer_id":999}]
    }));
    assert_eq!(invalid["isError"], true);
    assert_eq!(LocalDocumentStore.load(path).unwrap(), document);

    let edited = editor.text(
        "edit_map",
        json!({
            "map_id":id, "expected_revision":created["revision"],
            "changes":[
                {"op":"move", "layer_id":mooring, "to":[299,225]},
                {"op":"put", "layer_id":pass, "feature":{"name":"Pass", "art":{"kind":"stroke",
                    "brush":"path", "through":[[132,160],[205,116],"Mooring"]}}},
                {"op":"rename", "layer_id":title, "name":"New Quay"}
            ]
        }),
    );
    assert_ne!(edited["revision"], created["revision"]);
    let conflict = mcp.call(
        "edit_map",
        json!({
            "map_id":id, "expected_revision":created["revision"],
            "changes":[{"op":"remove", "layer_id":title}]
        }),
    );
    assert_eq!(conflict["isError"], true);
    assert!(
        conflict["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Revision conflict")
    );

    let atlas = LocalDocumentStore.load(path).unwrap();
    assert_eq!(
        atlas.layers[1].landmarks[0].id,
        document.layers[1].landmarks[0].id
    );
    assert_eq!(
        atlas.layers[1].landmarks[0].position,
        Point::new(299., 225.)
    );
    assert_eq!(
        atlas.layers[2].strokes.iter().next().unwrap().points.last(),
        Some(&Point::new(299., 225.))
    );
    assert_eq!(atlas.layers[3].labels[0].text, "New Quay");

    let cropped = mcp.call("preview_map", json!({"map_id":id, "layer_id":mooring}));
    assert_ne!(cropped["isError"], true);
    assert_ne!(cropped["content"][1]["data"], preview["content"][1]["data"]);
    let exported = mcp.text("export_map", json!({"map_id":id}));
    let svg = std::fs::read_to_string(exported["svg_path"].as_str().unwrap()).unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("</svg>"));
    assert_eq!(exported["revision"], edited["revision"]);
    assert_eq!(
        mcp.call("inspect_map", json!({"map_id":"../world"}))["isError"],
        true
    );
}

#[test]
fn mcp_uses_native_road_and_river_defaults_and_places_rotated_labels() {
    let root = Sandbox::new();
    let mut mcp = Client::connect(&root.0);
    let created = mcp.text(
        "create_map",
        json!({
            "name":"Valley",
            "features":[
                {"name":"River", "art":{"kind":"stroke", "brush":"river", "material":"lake", "through":[[90,120],[340,120]]}},
                {"name":"Road", "art":{"kind":"stroke", "brush":"path", "through":[[90,190],[340,190]]}},
                {"name":"Footpath", "art":{"kind":"stroke", "brush":"path", "size":30, "through":[[90,230],[340,230]]}},
                {"name":"Valley label", "art":{"kind":"label", "text":"Valley", "at":[200,160]}}
            ]
        }),
    );
    let path = Path::new(created["atlas_path"].as_str().unwrap());
    let document = LocalDocumentStore.load(path).unwrap();
    for (layer, brush) in document.layers.iter().zip([Brush::River, Brush::Path]) {
        let stroke = layer.strokes.iter().next().unwrap();
        assert_eq!(stroke.brush, brush);
        assert_eq!(stroke.size, 110.);
        assert_eq!(stroke.strength, 0.85);
    }
    assert_eq!(
        document.layers[0]
            .strokes
            .iter()
            .next()
            .unwrap()
            .material
            .as_deref(),
        Some("lake")
    );
    assert_eq!(document.layers[2].strokes.iter().next().unwrap().size, 30.);
    assert_eq!(document.layers[3].labels[0].rotation, 0.);

    let label_layer = document.layers[3].id;
    let label_id = document.layers[3].labels[0].id;
    let map_id = &created["map_id"];
    let export = mcp.text("export_map", json!({"map_id":map_id}));
    let svg = std::fs::read_to_string(export["svg_path"].as_str().unwrap()).unwrap();
    assert!(svg.contains(r##"fill="#ccc9ba""##));
    let edited = mcp.text(
        "edit_map",
        json!({
            "map_id":map_id, "expected_revision":created["revision"],
            "changes":[{"op":"put", "layer_id":label_layer, "feature":{
                "name":"Valley label", "art":{"kind":"label", "text":"Valley", "at":[200,160],"rotation":-25}
            }}]
        }),
    );
    let document = LocalDocumentStore.load(path).unwrap();
    assert_eq!(document.layers[3].labels[0].rotation, -25.);
    assert_eq!(document.layers[3].labels[0].id, label_id);
    let invalid = mcp.call(
        "edit_map",
        json!({
            "map_id":map_id, "expected_revision":edited["revision"],
            "changes":[{"op":"put", "layer_id":label_layer, "feature":{
                "name":"Valley label", "art":{"kind":"label", "text":"Valley", "at":[200,160],"rotation":181}
            }}]
        }),
    );
    assert_eq!(invalid["isError"], true);
    assert_eq!(LocalDocumentStore.load(path).unwrap(), document);
}

#[test]
fn breadth_first_areas_paint_outward_and_reject_an_invalid_origin_without_editing() {
    let root = Sandbox::new();
    let mut mcp = Client::connect(&root.0);
    let outline = json!([
        [0, 0],
        [70, 0],
        [70, 140],
        [130, 140],
        [130, 0],
        [200, 0],
        [200, 220],
        [0, 220]
    ]);
    let make_feature = |from: [i32; 2]| {
        json!({"name": "Bay", "art": {
            "kind": "area", "brush": "water", "material": "ocean", "outline": outline,
            "drawing": {"approach": "breadth_first", "from": from}
        }})
    };
    let created = mcp.text(
        "create_map",
        json!({"name": "Bay", "features": [make_feature([35, 30])]}),
    );
    let map_id = &created["map_id"];
    let path = Path::new(created["atlas_path"].as_str().unwrap());
    let document = LocalDocumentStore.load(path).unwrap();
    let layer = &document.layers[0];
    assert_eq!(document.layers.len(), 1);
    let strokes: Vec<_> = layer.strokes.iter().collect();
    let first = strokes[0].points.first().unwrap();
    assert!((first.x - 35.).abs() < 5. && (first.y - 30.).abs() < 5.);
    assert!(
        strokes
            .windows(2)
            .all(|pair| pair[0].creation_id < pair[1].creation_id)
    );
    assert!(strokes.iter().all(|stroke| {
        stroke.brush == Brush::Water && stroke.material.as_deref() == Some("ocean")
    }));
    let bridge = strokes
        .iter()
        .position(|stroke| {
            stroke
                .points
                .iter()
                .any(|p| p.x > 90. && p.x < 110. && p.y > 140.)
        })
        .unwrap();
    let other_arm = strokes
        .iter()
        .position(|stroke| stroke.points.iter().any(|p| p.x > 155. && p.y < 65.))
        .unwrap();
    assert!(bridge < other_arm, "the wave must go around the gap");

    let failed = mcp.call(
        "edit_map",
        json!({"map_id": map_id, "expected_revision": created["revision"], "changes": [
            {"op": "put", "layer_id": layer.id, "feature": make_feature([100, 35])}
        ]}),
    );
    assert_eq!(failed["isError"], true);
    assert!(
        failed["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("inside the area outline")
    );
    assert_eq!(LocalDocumentStore.load(path).unwrap(), document);

    let edited = mcp.text(
        "edit_map",
        json!({"map_id": map_id, "expected_revision": created["revision"], "changes": [
            {"op": "put", "layer_id": layer.id, "feature": make_feature([165, 35])}
        ]}),
    );
    assert_ne!(edited["revision"], created["revision"]);
    let redrawn = LocalDocumentStore.load(path).unwrap();
    let layer = &redrawn.layers[0];
    let first = layer.strokes.iter().next().unwrap().points.first().unwrap();
    assert!((first.x - 165.).abs() < 5. && (first.y - 35.).abs() < 5.);
}
