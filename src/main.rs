mod app;
mod mcp;

#[global_allocator]
static ALLOCATOR: elmapa::allocation::CountingAllocator = elmapa::allocation::CountingAllocator;

fn main() {
    let mut args = std::env::args_os().skip(1);
    if args.next().is_some_and(|argument| argument == "--mcp") {
        let result = match (args.next(), args.next()) {
            (Some(root), None) => mcp::serve(root.into()),
            _ => Err("Usage: elmapa --mcp <output-directory>".into()),
        };
        if let Err(error) = result {
            eprintln!("Elmapa MCP: {error}");
            std::process::exit(1);
        }
    } else {
        app::run();
    }
}
