// Registry/aliases adapted from Arborium 2.18.2, MIT OR Apache-2.0.
// Copyright (c) Amos Wenger and contributors. See third-party/highlighting.md.
// Grammars/queries remain pinned upstream crates, not copied parsers.
// Cargo features are additive: explicit reduced consumers must fail instead
// of silently linking excluded grammars through another workspace dependency.
#[cfg(all(
    feature = "reduced",
    any(
        feature = "cobol",
        feature = "d",
        feature = "hlsl",
        feature = "fsharp",
        feature = "ocaml",
        feature = "idris",
        feature = "odin",
        feature = "c-sharp",
        feature = "perl",
        feature = "ruby",
        feature = "elixir",
        feature = "vb",
        feature = "php"
    )
))]
compile_error!(
    "syntax-reduced conflicts with full/optional syntax features; disable default features on every MarkNoria dependency"
);
use std::sync::OnceLock;
use tree_sitter_highlight::HighlightConfiguration;
pub const CAPTURES: &[&str] = &[
    "attribute",
    "boolean",
    "comment",
    "constant",
    "constructor",
    "embedded",
    "error",
    "escape",
    "function",
    "keyword",
    "label",
    "markup",
    "module",
    "number",
    "operator",
    "property",
    "punctuation",
    "string",
    "tag",
    "type",
    "variable",
    // Older Neovim queries use these instead of the newer dotted captures.
    "conditional",
    "repeat",
    "include",
    "exception",
    "storageclass",
    "character",
    "float",
    "method",
    "namespace",
    "field",
    "markup.heading",
    "markup.bold",
    "markup.strong",
    "markup.italic",
    "markup.emphasis",
    "text.title",
    "text.strong",
    "text.emphasis",
    "text.literal",
    "text.uri",
    "text.reference",
];
fn metadata_free(query: &str) -> String {
    query
        .replace("@nospell", "")
        .replace("@spell", "")
        .replace("@conceal", "")
}

// Adapt obsolete query node shapes to the pinned grammar's node-types schema.
// These targeted fixes keep the captures, rather than dropping a whole language.
fn compatible_query(name: &str, query: &str) -> String {
    let mut query = metadata_free(query);
    let patches: &[(&str, &str)] = match name {
        #[cfg(feature = "fsharp")]
        "fsharp" => &[
            (
                "(_type\n  (long_identifier (identifier) @type.builtin))",
                "(simple_type (long_identifier (identifier) @type.builtin))",
            ),
            (
                "(_type\n         (long_identifier\n           (identifier) @attribute))",
                "(simple_type (long_identifier (identifier) @attribute))",
            ),
        ],
        #[cfg(feature = "perl")]
        "perl" => &[
            (
                "(postfix_deref [\"@\" \"$#\" ] @variable.array \"*\" @variable.array)",
                "[(array_deref_expression \"@\" @variable.array \"*\" @variable.array) (arraylen_deref_expression \"$#\" @variable.array \"*\" @variable.array)]",
            ),
            (
                "(postfix_deref \"%\" @variable.hash \"*\" @variable.hash)",
                "(hash_deref_expression \"%\" @variable.hash \"*\" @variable.hash)",
            ),
            (
                "(slices\n  hashref:_ [ \"@\" \"%\" ] @variable.hash )",
                "[(slice_expression hashref: (_) \"@\" @variable.hash) (keyval_expression hashref: (_) \"%\" @variable.hash)]",
            ),
            (
                "(slices\n  arrayref:_  [ \"@\" \"%\" ] @variable.array )",
                "[(slice_expression arrayref: (_) \"@\" @variable.array) (keyval_expression arrayref: (_) \"%\" @variable.array)]",
            ),
        ],
        "solidity" => &[(
            "(struct_expression type: ((expression (identifier)) @type .))",
            "(struct_expression type: (expression (identifier) @type))",
        )],
        _ => &[],
    };
    for (before, after) in patches {
        debug_assert!(query.contains(before), "Stale {name} query adapter");
        query = query.replace(before, after);
    }
    query
}

pub fn alias(name: &str) -> &str {
    match name {
        "adoc" => "asciidoc",
        "assembly" => "asm",
        "bat" => "batch",
        "bazel" => "starlark",
        "bzl" => "starlark",
        "c++" => "cpp",
        "cbl" => "cobol",
        "cfg" => "ini",
        "cjs" => "javascript",
        "cl" => "commonlisp",
        "clj" => "clojure",
        "cmd" => "batch",
        "cob" => "cobol",
        "conf" => "ini",
        "cpy" => "cobol",
        "cs" => "c-sharp",
        "csharp" => "c-sharp",
        "cts" => "typescript",
        "cxx" => "cpp",
        "dlang" => "d",
        "docker" => "dockerfile",
        "el" => "elisp",
        "emacs-lisp" => "elisp",
        "erl" => "erlang",
        "ex" => "elixir",
        "exs" => "elixir",
        "f#" => "fsharp",
        "frag" => "glsl",
        "fs" => "fsharp",
        "golang" => "go",
        "gql" => "graphql",
        "gsh" => "groovy",
        "gvy" => "groovy",
        "gy" => "groovy",
        "h" => "c",
        "hpp" => "cpp",
        "hs" => "haskell",
        "htm" => "html",
        "idr" => "idris",
        "j2" => "jinja2",
        "jinja" => "jinja2",
        "jl" => "julia",
        "js" => "javascript",
        "jsonc" => "json",
        "jsx" => "javascript",
        "kt" => "kotlin",
        "kts" => "kotlin",
        "lisp" => "commonlisp",
        "m" => "matlab",
        "md" => "markdown",
        "mdx" => "markdown",
        "mjs" => "javascript",
        "ml" => "ocaml",
        "mm" => "objc",
        "mts" => "typescript",
        "mysql" => "sql",
        "nasm" => "x86asm",
        "objective-c" => "objc",
        "opa" => "rego",
        "patch" => "diff",
        "pbtxt" => "textproto",
        "pl" => "perl",
        "pm" => "perl",
        "postgres" => "sql",
        "postgresql" => "sql",
        "pro" => "prolog",
        "protobuf" => "proto",
        "ps" => "postscript",
        "ps1" => "powershell",
        "pwsh" => "powershell",
        "py" => "python",
        "py3" => "python",
        "python3" => "python",
        "rb" => "ruby",
        "res" => "rescript",
        "rkt" => "scheme",
        "rlang" => "r",
        "rq" => "sparql",
        "rs" => "rust",
        "sass" => "scss",
        "scm" => "query",
        "sh" => "bash",
        "shell" => "bash",
        "sol" => "solidity",
        "sqlite" => "sql",
        "ss" => "scheme",
        "sv" => "verilog",
        "svg" => "xml",
        "systemverilog" => "verilog",
        "terraform" => "hcl",
        "textpb" => "textproto",
        "tf" => "hcl",
        "tla" => "tlaplus",
        "ts" => "typescript",
        "typ" => "typst",
        "ua" => "uiua",
        "v" => "verilog",
        "vbnet" => "vb",
        "vert" => "glsl",
        "vhd" => "vhdl",
        "viml" => "vim",
        "vimscript" => "vim",
        "visualbasic" => "vb",
        "wasm-interface" => "wit",
        "x86" => "x86asm",
        "xsl" => "xml",
        "xslt" => "xml",
        "yml" => "yaml",
        _ => name,
    }
}

macro_rules! grammar {
    ($module:ident,$name:literal) => {{
        static CONFIG: OnceLock<Option<HighlightConfiguration>> = OnceLock::new();
        CONFIG
            .get_or_init(|| {
                let query = compatible_query($name, &$module::HIGHLIGHTS_QUERY);
                let mut config = HighlightConfiguration::new(
                    $module::language().into(),
                    $name,
                    &query,
                    $module::INJECTIONS_QUERY,
                    $module::LOCALS_QUERY,
                )
                .map_err(|_error| {
                    #[cfg(debug_assertions)]
                    eprintln!("Bundled query {}: {_error:?}", $name);
                })
                .ok()?;
                config.configure(CAPTURES);
                Some(config)
            })
            .as_ref()
    }};
}

macro_rules! registry {
    ($( $(#[$condition:meta])* $name:literal => $module:ident ),* $(,)?) => {
        pub const LANGUAGES: &[&str] = &[$( $(#[$condition])* $name ),*];
        pub fn configuration(name: &str) -> Option<&'static HighlightConfiguration> {
            match alias(name) { $( $(#[$condition])* $name => grammar!($module, $name), )* _ => None }
        }
    };
}
// One registry owns both dispatch and advertised coverage. Excluded languages
// have no dependency or compiled initializer; aliases/injections resolve None.
registry! {
    "ada" => arborium_ada,
    "agda" => arborium_agda,
    "asciidoc" => arborium_asciidoc,
    "asm" => arborium_asm,
    "awk" => arborium_awk,
    "bash" => arborium_bash,
    "batch" => arborium_batch,
    "c" => arborium_c,
    #[cfg(feature = "c-sharp")]
    "c-sharp" => arborium_c_sharp,
    "caddy" => arborium_caddy,
    "capnp" => arborium_capnp,
    "cedar" => arborium_cedar,
    "cedarschema" => arborium_cedarschema,
    "clojure" => arborium_clojure,
    "cmake" => arborium_cmake,
    #[cfg(feature = "cobol")]
    "cobol" => arborium_cobol,
    "commonlisp" => arborium_commonlisp,
    "cpp" => arborium_cpp,
    "css" => arborium_css,
    #[cfg(feature = "d")]
    "d" => arborium_d,
    "dart" => arborium_dart,
    "devicetree" => arborium_devicetree,
    "diff" => arborium_diff,
    "dockerfile" => arborium_dockerfile,
    "dot" => arborium_dot,
    "elisp" => arborium_elisp,
    #[cfg(feature = "elixir")]
    "elixir" => arborium_elixir,
    "elm" => arborium_elm,
    "erlang" => arborium_erlang,
    "fish" => arborium_fish,
    #[cfg(feature = "fsharp")]
    "fsharp" => arborium_fsharp,
    "gitattributes" => arborium_gitattributes,
    "gleam" => arborium_gleam,
    "glsl" => arborium_glsl,
    "go" => arborium_go,
    "graphql" => arborium_graphql,
    "groovy" => arborium_groovy,
    "haskell" => arborium_haskell,
    "hcl" => arborium_hcl,
    #[cfg(feature = "hlsl")]
    "hlsl" => arborium_hlsl,
    "html" => arborium_html,
    #[cfg(feature = "idris")]
    "idris" => arborium_idris,
    "ini" => arborium_ini,
    "java" => arborium_java,
    "javascript" => arborium_javascript,
    "jinja2" => arborium_jinja2,
    "jq" => arborium_jq,
    "jsdoc" => arborium_jsdoc,
    "json" => arborium_json,
    "julia" => arborium_julia,
    "just" => arborium_just,
    "kconfig" => arborium_kconfig,
    "kdl" => arborium_kdl,
    "kotlin" => arborium_kotlin,
    "lean" => arborium_lean,
    "lua" => arborium_lua,
    "make" => arborium_make,
    "markdown" => arborium_markdown,
    "matlab" => arborium_matlab,
    "meson" => arborium_meson,
    "ninja" => arborium_ninja,
    "nix" => arborium_nix,
    "objc" => arborium_objc,
    #[cfg(feature = "ocaml")]
    "ocaml" => arborium_ocaml,
    #[cfg(feature = "odin")]
    "odin" => arborium_odin,
    #[cfg(feature = "perl")]
    "perl" => arborium_perl,
    #[cfg(feature = "php")]
    "php" => arborium_php,
    "postscript" => arborium_postscript,
    "powershell" => arborium_powershell,
    "prolog" => arborium_prolog,
    "proto" => arborium_proto,
    "python" => arborium_python,
    "query" => arborium_query,
    "r" => arborium_r,
    "regex" => arborium_regex,
    "rego" => arborium_rego,
    "rescript" => arborium_rescript,
    "ron" => arborium_ron,
    #[cfg(feature = "ruby")]
    "ruby" => arborium_ruby,
    "rust" => arborium_rust,
    "scala" => arborium_scala,
    "scheme" => arborium_scheme,
    "scss" => arborium_scss,
    "solidity" => arborium_solidity,
    "sparql" => arborium_sparql,
    "sql" => arborium_sql,
    "ssh-config" => arborium_ssh_config,
    "starlark" => arborium_starlark,
    "styx" => arborium_styx,
    "svelte" => arborium_svelte,
    "swift" => arborium_swift,
    "textproto" => arborium_textproto,
    "thrift" => arborium_thrift,
    "tlaplus" => arborium_tlaplus,
    "toml" => arborium_toml,
    "tsx" => arborium_tsx,
    "typescript" => arborium_typescript,
    "typst" => arborium_typst,
    #[cfg(feature = "vb")]
    "vb" => arborium_vb,
    "verilog" => arborium_verilog,
    "vhdl" => arborium_vhdl,
    "vim" => arborium_vim,
    "vue" => arborium_vue,
    "wit" => arborium_wit,
    "x86asm" => arborium_x86asm,
    "xml" => arborium_xml,
    "yaml" => arborium_yaml,
    "yuri" => arborium_yuri,
    "zig" => arborium_zig,
    "zsh" => arborium_zsh,
}

#[cfg(test)]
mod tests {
    #[test]
    fn active_registry_matches_the_selected_profile_and_aliases() {
        let excluded = [
            "cobol", "d", "hlsl", "fsharp", "ocaml", "idris", "odin", "c-sharp", "perl", "ruby",
            "elixir", "vb", "php",
        ];
        assert_eq!(
            super::LANGUAGES
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            super::LANGUAGES.len()
        );
        #[cfg(feature = "full")]
        assert_eq!(super::LANGUAGES.len(), 110);
        #[cfg(feature = "reduced")]
        {
            assert_eq!(super::LANGUAGES.len(), 97);
            for name in excluded {
                assert!(
                    super::configuration(name).is_none(),
                    "Excluded grammar {name} linked"
                );
            }
        }
        for name in [
            "lean",
            "verilog",
            "v",
            "sv",
            "systemverilog",
            "rust",
            "python",
            "html",
            "javascript",
            "css",
            "regex",
            "jsdoc",
        ] {
            assert!(
                super::configuration(name).is_some(),
                "Retained grammar/alias missing: {name}"
            );
        }
        for (alias, name) in [
            ("cs", "c-sharp"),
            ("csharp", "c-sharp"),
            ("cbl", "cobol"),
            ("cob", "cobol"),
            ("cpy", "cobol"),
            ("dlang", "d"),
            ("ex", "elixir"),
            ("exs", "elixir"),
            ("fs", "fsharp"),
            ("f#", "fsharp"),
            ("idr", "idris"),
            ("ml", "ocaml"),
            ("pl", "perl"),
            ("pm", "perl"),
            ("rb", "ruby"),
            ("vbnet", "vb"),
            ("visualbasic", "vb"),
        ] {
            assert_eq!(super::alias(alias), name);
            assert_eq!(
                super::configuration(alias).is_some(),
                super::LANGUAGES.contains(&name),
                "Alias selected another grammar: {alias}"
            );
        }
        let _ = excluded;
    }
    #[test]
    fn bundled_queries_are_valid() {
        let failed: Vec<_> = super::LANGUAGES
            .iter()
            .filter(|name| super::configuration(name).is_none())
            .collect();
        assert!(failed.is_empty(), "Invalid queries: {failed:?}")
    }
}
