#![allow(clippy::module_inception)]
use indoc::formatdoc;
use itertools::Itertools;

mod attributes;
mod base;
mod capabilities;
mod common;
mod config;
mod functions;
mod parsing;
mod reformat;
mod render_to_dmmf;
mod renderer;
mod types;

#[allow(dead_code)]
pub enum Provider {
    Postgres,
    Mysql,
    Sqlite,
    SqlServer,
    Mongo,
}

fn with_header(dm: &str, provider: Provider, preview_features: &[&str]) -> String {
    let (provider, url) = match provider {
        Provider::Mongo => ("mongodb", "mongo"),
        Provider::Postgres => ("postgres", "postgresql"),
        Provider::Sqlite => ("sqlite", "file"),
        Provider::Mysql => ("mysql", "mysql"),
        Provider::SqlServer => ("sqlserver", "sqlserver"),
    };

    let preview_features = if preview_features.is_empty() {
        "".to_string()
    } else {
        format!(
            "previewFeatures = [{}]",
            preview_features.iter().map(|f| format!("\"{}\"", f)).join(", ")
        )
    };

    let header = formatdoc!(
        r#"
        datasource test {{
          provider = "{}"
          url = "{}://..."
        }}
        
        generator client {{
          provider = "prisma-client-js"
          {}
        }}
        "#,
        provider,
        url,
        preview_features
    );

    format!("{}\n{}", header, dm)
}
