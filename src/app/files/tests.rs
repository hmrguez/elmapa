use super::*;

mod export {
    use super::*;
    #[::core::prelude::v1::test]
    fn cancelled_atomic_export_preserves_existing_destination() {
        let root =
            std::env::temp_dir().join(format!("elmapa-export-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for format in [ExportFormat::Svg, ExportFormat::Png] {
            let path = root.join(format!("world.{}", format.extension()));
            std::fs::write(&path, b"existing export").unwrap();
            let token = Arc::new(AtomicU64::new(1));
            assert!(format.write(&path, &Document::demo(), (token, 0)).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"existing export");
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

mod lifecycle {
    use super::*;
    use elmapa::document::{Brush, Point, Stroke};
    fn fixture() -> (App, PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "elmapa-bevy-io-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let document = Document::blank();
        let mut library = Library::load(root.clone()).unwrap();
        library
            .checkpoint(
                AtlasRecord {
                    id: 1,
                    name: document.name.clone(),
                    source: None,
                    camera: Camera::for_document(&document),
                },
                &document,
            )
            .unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_message::<AppExit>();
        app.insert_resource(Assets::<Image>::default())
            .insert_resource(Jobs {
                started: true,
                ..default()
            });
        app.insert_resource(Studio {
            home: false,
            atlas: Some(1),
            library: Some(library),
            ..default()
        });
        app.world_mut().spawn(Window::default());
        app.add_systems(Update, poll);
        (app, root)
    }
    fn wait(app: &mut App) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.world().resource::<Jobs>().busy() {
            app.update();
            assert!(std::time::Instant::now() < deadline, "I/O did not complete");
            std::thread::yield_now();
        }
    }
    fn paint(app: &mut App) {
        let mut s = app.world_mut().resource_mut::<Studio>();
        s.editor.commit_stroke(Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Forest,
            points: vec![Point::new(30., 40.)].into(),
            size: 110.,
            strength: 0.85,
            seed: 42,
        });
        s.changed();
    }
    #[test]
    fn asynchronous_save_keeps_newer_edits_dirty_and_remembers_destination() {
        let (mut app, root) = fixture();
        let path = root.join("external.elmapa");
        app.world_mut()
            .resource_mut::<Jobs>()
            .request(Request::Save(Some(path.clone())));
        app.update();
        paint(&mut app);
        wait(&mut app);
        assert_eq!(
            LocalDocumentStore.load(&path).unwrap().layers[0]
                .strokes
                .len(),
            0
        );
        let s = app.world().resource::<Studio>();
        assert!(s.dirty);
        assert_eq!(s.path.as_ref(), Some(&path));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn home_checkpoints_latest_document_camera_preferences_and_reentry_keeps_history() {
        let (mut app, root) = fixture();
        paint(&mut app);
        {
            let mut s = app.world_mut().resource_mut::<Studio>();
            s.appearance = crate::app::state::Appearance::Paper;
            s.viewport.pan.x = 37.;
        }
        app.world_mut()
            .resource_mut::<Jobs>()
            .request(Request::Home);
        wait(&mut app);
        let s = app.world().resource::<Studio>();
        assert!(s.home);
        assert!(s.editor.can_undo());
        let library = Library::load(root.clone()).unwrap();
        assert!(library.shelf.paper_ui);
        assert_eq!(library.read(1).unwrap(), *s.editor.document());
        let mut jobs = app.world_mut().remove_resource::<Jobs>().unwrap();
        app.world_mut()
            .resource_mut::<Studio>()
            .act(crate::app::state::Action::Enter(1), &mut jobs);
        assert!(!app.world().resource::<Studio>().home);
        assert!(app.world().resource::<Studio>().editor.can_undo());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn external_save_can_rescue_edits_when_library_checkpointing_is_unavailable() {
        let (mut app, root) = fixture();
        paint(&mut app);
        std::fs::remove_dir_all(&root).unwrap();
        std::fs::write(&root, b"blocked library path").unwrap();
        let path = root.with_extension("external.elmapa");
        app.world_mut()
            .resource_mut::<Jobs>()
            .request(Request::Save(Some(path.clone())));
        wait(&mut app);
        assert_eq!(
            LocalDocumentStore.load(&path).unwrap(),
            *app.world().resource::<Studio>().editor.document()
        );
        assert!(!app.world().resource::<Studio>().dirty);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(root).unwrap();
    }
}
