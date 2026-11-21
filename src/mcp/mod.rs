//! Local stdio adapter for creating and revising editable Elmapa files.
mod plan;
mod store;

use base64::{Engine, engine::general_purpose::STANDARD};
use elmapa::{biome_packs, brush_materials, scene};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    schemars::{self, JsonSchema},
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use store::Store;
use tokio::sync::Semaphore;

pub fn serve(root: PathBuf) -> Result<(), String> {
    let server = Server {
        store: Store::new(&root)?,
        work: Arc::new(Semaphore::new(1)),
        tool_router: Server::tool_router(),
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?
        .block_on(async {
            let service = server
                .serve(rmcp::transport::stdio())
                .await
                .map_err(|error| error.to_string())?;
            service.waiting().await.map_err(|error| error.to_string())?;
            Ok(())
        })
}

#[derive(Clone)]
struct Server {
    store: Store,
    work: Arc<Semaphore>,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Inspect {
    /// Omit to discover biome packs, landmark asset IDs, and brush material IDs.
    map_id: Option<String>,
    /// Inspect only this layer, using its numeric layer ID.
    layer_id: Option<u64>,
    /// Offset from the bottom for the next page of map layers.
    offset: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Preview {
    map_id: String,
    /// Fit this layer's artwork, with neighboring artwork still visible.
    layer_id: Option<u64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Export {
    map_id: String,
}

#[tool_router]
impl Server {
    #[tool(
        description = "Make an editable atlas in one call from named areas, strokes, landmarks, and labels. Coordinates are world units, x right and y down. Features are ordered bottom to top.",
        annotations(destructive_hint = false, open_world_hint = false)
    )]
    async fn create_map(&self, Parameters(plan): Parameters<plan::Create>) -> CallToolResult {
        self.run(move |store| Ok(receipt(store.create(plan)?)))
            .await
    }

    #[tool(
        description = "Apply moves, renames, additions, replacements or removals as one edit. Use the last returned revision and stable layer IDs. Reissue a route with put to recalculate its waypoint anchors.",
        annotations(
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn edit_map(&self, Parameters(edit): Parameters<plan::Edit>) -> CallToolResult {
        self.run(move |store| Ok(receipt(store.edit(edit)?))).await
    }

    #[tool(
        description = "Inspect a map's revision, bounds and named layers without returning stroke geometry. Omit map_id to discover biome packs and asset IDs.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn inspect_map(&self, Parameters(query): Parameters<Inspect>) -> CallToolResult {
        self.run(move |store| {
            let result = match query.map_id {
                Some(id) => store.inspect(&id, query.layer_id, query.offset.unwrap_or(0))?,
                None if query.layer_id.is_none() && query.offset.is_none() => asset_catalog(),
                None => return Err("Supply map_id to inspect a layer or page.".into()),
            };
            Ok(receipt(result))
        })
        .await
    }

    #[tool(
        description = "See the current map as a 512×320 PNG. Fit the whole map or zoom into one layer by ID.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn preview_map(&self, Parameters(query): Parameters<Preview>) -> CallToolResult {
        self.run(move |store| {
            let (summary, png) = store.preview(&query.map_id, query.layer_id)?;
            Ok(CallToolResult::success(vec![
                ContentBlock::text(summary.to_string()),
                ContentBlock::image(STANDARD.encode(png), "image/png"),
            ]))
        })
        .await
    }

    #[tool(
        description = "Export the current atlas revision as a self-contained SVG and return its local path.",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn export_map(&self, Parameters(query): Parameters<Export>) -> CallToolResult {
        self.run(move |store| Ok(receipt(store.export(&query.map_id)?)))
            .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("elmapa", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Create a map in one call from a coarse plan, then preview it as an image. Use                  inspect_map for assets or feature IDs. Batch revisions with edit_map and pass the                  current revision. Maps are editable .elmapa files in the configured output directory.                  Export to SVG for sharing. Coordinates use x right and y down, with [1600, 1000]                  as the default planning canvas. Layers draw in plan order, bottom to top.",
            )
    }
}

impl Server {
    async fn run(
        &self,
        operation: impl FnOnce(Store) -> Result<CallToolResult, String> + Send + 'static,
    ) -> CallToolResult {
        let permit = self
            .work
            .clone()
            .acquire_owned()
            .await
            .expect("open semaphore");
        let store = self.store.clone();
        match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation(store)
        })
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => CallToolResult::error(vec![ContentBlock::text(error)]),
            Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        }
    }
}

fn receipt(value: Value) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(value.to_string())])
}

fn asset_catalog() -> Value {
    let materials = brush_materials::catalog()
        .iter()
        .map(|material| json!({"id": material.id, "brush": material.brush, "name": material.name}))
        .collect::<Vec<_>>();
    let landmarks = scene::landmark_registry()
        .iter()
        .map(|asset| json!({"id": asset.kind.key(), "name": asset.name}))
        .collect::<Vec<_>>();
    json!({
        "materials": materials,
        "landmarks": landmarks,
        "biome_packs": biome_packs::catalog(),
    })
}
