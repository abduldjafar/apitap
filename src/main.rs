use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use minijinja::{Environment, Error, path_loader};
use minijinja::value::{Kwargs, Value};
use apitap::errors::Result;
use walkdir::WalkDir;

#[derive(Default, Debug)]
struct TemplateMeta {
    sink_name: String,
    source_name: String,

}

fn main() -> Result<()> {
    let sql_paths = list_sql_templates("pipelines")?;

    let mut env = Environment::new();
    env.set_loader(path_loader("pipelines")); // root is "pipelines/"

    let meta = Arc::new(Mutex::new(TemplateMeta::default()));
    let meta_for_fn = Arc::clone(&meta);

    env.add_function("sink", move |kwargs: Kwargs| -> std::result::Result<Value, Error> {
        let name: String = kwargs.get("name")?;
        meta_for_fn.lock().unwrap().sink_name = name;
        Ok(Value::from(""))
    });

    env.add_function("use_source", {
        let meta_for_fn = Arc::clone(&meta);
        move |name: String| -> std::result::Result<Value, Error> {
            meta_for_fn.lock().unwrap().source_name = name.clone();
            Ok(Value::from(name)) // echo so it still renders in SQL
        }
    });

    for name in sql_paths {
        // optional: clear previous capture
        meta.lock().unwrap().sink_name.clear();

        // 👇 Use the relative template name directly (e.g., "placeholder/post.sql")
        let tmpl = env.get_template(&name)?;
        let rendered = tmpl.render(())?;

        println!("\n=== {name} ===");
        println!("Rendered SQL:\n{}", rendered.trim());
        println!("Captured sink name: {:?}", meta.lock().unwrap().sink_name);
        println!("Captured source name: {}", meta.lock().unwrap().source_name);

    }

    Ok(())
}

fn list_sql_templates(root: impl AsRef<Path>) -> Result<Vec<String>> {
    let root = root.as_ref();
    let mut out = Vec::new();

    for entry_res in WalkDir::new(root) {
        let entry = match entry_res {
            Ok(e) => e,
            Err(_) => continue, // skip unreadable entries
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();

        // only .sql (case-insensitive)
        let is_sql = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("sql"))
            .unwrap_or(false);
        if !is_sql {
            continue;
        }

        // make relative to root; if it fails, skip
        let rel: &Path = match path.strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // convert to forward slashes for Minijinja names
        let name = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        out.push(name);
    }

    out.sort();
    Ok(out)
}