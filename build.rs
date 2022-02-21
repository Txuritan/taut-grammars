use std::{path::PathBuf, io::{BufWriter, Write as _}, fs, env, collections::HashMap};

fn main() {
    let cur_dir = env::current_dir().unwrap();
    let grammars_dir = cur_dir.join("grammars");

    // Compile all grammars

    let mut parser_c_config = cc::Build::new();
    parser_c_config.include(&cur_dir);
    parser_c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs");

    let mut scanner_c_config = cc::Build::new();
    scanner_c_config.include(&cur_dir);
    scanner_c_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable");

    let mut scanner_cpp_config = cc::Build::new();
    scanner_cpp_config.cpp(true);
    scanner_cpp_config.include(&cur_dir);
    scanner_cpp_config
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable");

    for entry in fs::read_dir(&grammars_dir).unwrap() {
        let entry = entry.unwrap();
        let meta = entry.metadata().unwrap();

        if !meta.is_dir() {
            continue;
        }

        let path = entry.path();

        let src_path = path.join("src");

        parser_c_config.include(&src_path);
        scanner_c_config.include(&src_path);
        scanner_cpp_config.include(&src_path);

        let parser_path = src_path.join("parser.c");
        if parser_path.exists() {
            parser_c_config.file(&parser_path);
            println!("cargo:rerun-if-changed={}", parser_path.to_str().unwrap());
        }

        let scanner_c_path = src_path.join("scanner.c");
        if scanner_c_path.exists() {
            scanner_c_config.file(&scanner_c_path);
            println!("cargo:rerun-if-changed={}", scanner_c_path.to_str().unwrap());
        }

        let scanner_cpp_path = src_path.join("scanner.cc");
        if scanner_cpp_path.exists() {
            scanner_cpp_config.file(&scanner_cpp_path);
            println!("cargo:rerun-if-changed={}", scanner_cpp_path.to_str().unwrap());
        }
    }

    parser_c_config.compile("parser_c");
    scanner_c_config.compile("scanner_c");
    scanner_cpp_config.compile("scanner_cpp");

    // Read `gitmodules`

    let modules_path = cur_dir.join(".gitmodules");
    let modules_contents = fs::read_to_string(&modules_path).unwrap();

    let mut lines = modules_contents.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty());

    let mut modules = HashMap::new();

    while let Some((_group, path, url)) = lines.next().and_then(|a| lines.next().and_then(|b| lines.next().map(|c| (a, b, c)))) {
        let path = path.trim_start_matches("path = ");
        let url = url.trim_start_matches("url = ");

        if let Some(name) = path.split('/').filter(|s| !s.is_empty()).last() {
            modules.insert(name.to_string(), url.to_string());
        }
    }

    // Generate grammar binding file

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut buf = BufWriter::new(Vec::new());

    writeln!(&mut buf, r#"use tree_sitter::Language;"#).unwrap();

    writeln!(&mut buf).unwrap();

    writeln!(&mut buf, r#"#[cfg(feature = "tree-sitter-highlight")]"#).unwrap();
    writeln!(&mut buf, r#"use tree_sitter_highlight::HighlightConfiguration;"#).unwrap();

    writeln!(&mut buf).unwrap();

    for entry in fs::read_dir(&grammars_dir).unwrap() {
        let entry = entry.unwrap();
        let meta = entry.metadata().unwrap();

        if !meta.is_dir() {
            continue;
        }

        let path = entry.path();

        let queries_dir = path.join("queries");

        let folder_name = path.file_name().unwrap().to_str().unwrap();
        let module_name = folder_name.replace('-', "_");

        let head_path = cur_dir.join(".git").join("modules").join("grammars").join(folder_name).join("refs").join("heads");
        let master_path = head_path.join("master");
        let main_path = head_path.join("main");

        let ref_path = if master_path.exists() {
            Some(master_path)
        } else if main_path.exists() {
            Some(main_path)
        } else {
            None
        };

        let ref_contents = if let Some(path) = ref_path {
            fs::read_to_string(path).unwrap().lines().next().map(|l| l.trim().to_string())
        } else {
            None
        };

        let has_queries_highlights = queries_dir.join("highlights.scm").exists();
        let has_queries_injections = queries_dir.join("injections.scm").exists();
        let has_queries_locals = queries_dir.join("locals.scm").exists();

        if let Some(url) = modules.get(folder_name) {
            writeln!(&mut buf, r#"/// Binds for the [`{}`]({}) tree-sitter grammar."#, folder_name, url).unwrap();

            writeln!(&mut buf, r#"///"#).unwrap();

            if let Some(ref_contents) = ref_contents {
                writeln!(&mut buf, r#"/// # GENERATED INFO"#).unwrap();
                writeln!(&mut buf, r#"///"#).unwrap();
                writeln!(&mut buf, r#"/// REPO: <{}> <br/>"#, url).unwrap();
                writeln!(&mut buf, r#"/// REF: {}"#, ref_contents).unwrap();
            } else {
                writeln!(&mut buf, r#"/// ERRO: Unable to get repo ref"#).unwrap();
            }
        }
        writeln!(&mut buf, r#"pub mod {} {{"#, module_name).unwrap();
        writeln!(&mut buf, r#"    use super::*;"#).unwrap();

        writeln!(&mut buf).unwrap();

        writeln!(&mut buf, r#"    extern "C" {{"#).unwrap();
        writeln!(&mut buf, r#"        fn tree_sitter_{}() -> Language;"#, module_name).unwrap();
        writeln!(&mut buf, r#"    }}"#).unwrap();

        writeln!(&mut buf).unwrap();

        writeln!(&mut buf, r#"    pub fn language() -> Language {{"#).unwrap();
        writeln!(&mut buf, r#"        unsafe {{ tree_sitter_{}() }}"#, module_name).unwrap();
        writeln!(&mut buf, r#"    }}"#).unwrap();

        writeln!(&mut buf).unwrap();

        writeln!(&mut buf, r#"    pub const GRAMMAR: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/grammars/{}/grammar.js"));"#, folder_name).unwrap();
        writeln!(&mut buf, r#"    pub const NODE_TYPES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/grammars/{}/src/node-types.json"));"#, folder_name).unwrap();

        if has_queries_highlights {
            writeln!(&mut buf, r#"    pub const HIGHLIGHTS_QUERY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/grammars/{}/queries/highlights.scm"));"#, folder_name).unwrap();
        }
        if has_queries_injections {
            writeln!(&mut buf, r#"    pub const INJECTIONS_QUERY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/grammars/{}/queries/injections.scm"));"#, folder_name).unwrap();
        }
        if has_queries_locals {
            writeln!(&mut buf, r#"    pub const LOCALS_QUERY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/grammars/{}/queries/locals.scm"));"#, folder_name).unwrap();
        }

        writeln!(&mut buf).unwrap();

        writeln!(&mut buf, r#"    #[cfg(feature = "tree-sitter-highlight")]"#).unwrap();
        writeln!(&mut buf, r#"    pub fn config() -> Result<HighlightConfiguration, tree_sitter::QueryError> {{"#).unwrap();
        writeln!(&mut buf, r#"        Ok(HighlightConfiguration::new("#).unwrap();
        writeln!(&mut buf, r#"            language(),"#).unwrap();
        writeln!(&mut buf, r#"            {},"#, if has_queries_highlights { "HIGHLIGHTS_QUERY" } else { r#""""# }).unwrap();
        writeln!(&mut buf, r#"            {},"#, if has_queries_highlights { "INJECTIONS_QUERY" } else { r#""""# }).unwrap();
        writeln!(&mut buf, r#"            {},"#, if has_queries_highlights { "LOCALS_QUERY" } else { r#""""# }).unwrap();
        writeln!(&mut buf, r#"        )?)"#).unwrap();
        writeln!(&mut buf, r#"    }}"#).unwrap();

        writeln!(&mut buf, r#"}}"#).unwrap();

        writeln!(&mut buf).unwrap();
    }

    fs::write(out_dir.join("grammars.rs"), buf.into_inner().unwrap()).unwrap();
}
