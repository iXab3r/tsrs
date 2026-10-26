// EyeAuras fork: `alwaysStrict: false` compiles standalone scripts in sloppy mode (TS 5 semantics, EYEAURAS-FORK.md).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tsrs_compiler::{
    Context, EmitOptions, Program, ProgramOptions, new_compiler_host, new_program,
};
use tsrs_core::{P, Tristate};
use tsrs_vfs::{FS, vfstest};

// Strict-only constructs: `with`, a strict reserved word, `delete` of an identifier, assignment to `arguments`.
const SLOPPY: &str = "var o = { a: 1 };\nwith (o) { a; }\nvar implements = 1;\nvar x = 1;\ndelete x;\nfunction f() { arguments = []; return arguments.callee; }\n";
const STRICT: &str = "\"use strict\";\nvar o = { a: 1 };\nwith (o) { a; }\n";
const STRICT_FUNCTION: &str =
    "function g() { \"use strict\"; var o = {}; with (o) { } }\nwith ({}) { }\n";
const CLASS: &str = "class C { m() { var o = {}; with (o) { } } }\nwith ({}) { }\n";

struct Output {
    syntactic: BTreeMap<String, Vec<i32>>,
    bind: BTreeMap<String, Vec<i32>>,
    program: Vec<i32>,
    emitted: BTreeMap<String, String>,
}

fn compile(always_strict: Tristate, files: &[(&str, &str)]) -> Output {
    let fs: Arc<dyn FS> = Arc::new(vfstest::from_map(files.iter().map(|&(k, v)| (k, v)), true));
    let host = new_compiler_host("/p", fs, "", None, None, None);
    let options = tsrs_core::CompilerOptions {
        no_lib: Tristate::True,
        allow_js: Tristate::True,
        out_dir: "/out".to_string(),
        always_strict,
        ..Default::default()
    };
    let names = files.iter().map(|(name, _)| (*name).to_string()).collect();
    let config = tsrs_tsoptions::new_parsed_command_line(
        P::new(options),
        names,
        vec![],
        tsrs_core::tspath::ComparePathsOptions::default(),
    );
    let mut opts = ProgramOptions::new(P::new(config), host);
    opts.single_threaded = Tristate::True;
    let program: &'static Program = new_program(opts);
    let ctx = Context::background();
    let mut out = Output {
        syntactic: BTreeMap::new(),
        bind: BTreeMap::new(),
        program: program.get_program_diagnostics().iter().map(|d| d.code()).collect(),
        emitted: BTreeMap::new(),
    };
    for &(name, _) in files {
        let file = program.get_source_file(name).unwrap();
        let codes = |diags: Vec<P<tsrs_ast::Diagnostic>>| diags.iter().map(|d| d.code()).collect();
        out.syntactic
            .insert(name.to_string(), codes(program.get_syntactic_diagnostics(&ctx, Some(file))));
        out.bind.insert(name.to_string(), codes(program.get_bind_diagnostics(&ctx, Some(file))));
    }
    let emitted = Mutex::new(BTreeMap::new());
    let write = |name: &str, text: &str, _: &mut tsrs_compiler::WriteFileData| {
        emitted.lock().unwrap().insert(name.to_string(), text.to_string());
        Ok(())
    };
    program.emit(&ctx, &EmitOptions { write_file: Some(&write), ..Default::default() });
    out.emitted = emitted.into_inner().unwrap();
    out
}

fn has_use_strict(text: &str) -> bool {
    text.contains("use strict")
}

#[test]
fn sloppy_scripts_compile_without_strict_mode() {
    let out = compile(Tristate::False, &[("/p/a.js", SLOPPY), ("/p/b.ts", SLOPPY)]);
    assert!(out.program.is_empty(), "{:?}", out.program);
    for name in ["/p/a.js", "/p/b.ts"] {
        assert_eq!(out.syntactic[name], Vec::<i32>::new(), "{name}");
        assert_eq!(out.bind[name], Vec::<i32>::new(), "{name}");
    }
    assert_eq!(out.emitted.len(), 2, "{:?}", out.emitted.keys());
    for text in out.emitted.values() {
        assert!(!has_use_strict(text), "{text}");
        assert!(text.contains("with (o)"), "{text}");
    }
}

#[test]
fn legacy_octal_stays_a_syntax_error_and_is_emitted_as_decimal() {
    let out = compile(Tristate::False, &[("/p/a.js", "var n = 010;\n")]);
    assert_eq!(out.syntactic["/p/a.js"], vec![1121]);
    // The printer writes the value of a legacy octal literal (existing behavior, independent of alwaysStrict).
    assert_eq!(out.emitted["/out/a.js"], "var n = 8;\n");
}

#[test]
fn use_strict_prologues_and_classes_stay_strict() {
    let out = compile(
        Tristate::False,
        &[
            ("/p/a.js", STRICT),
            ("/p/b.ts", STRICT),
            ("/p/f.js", STRICT_FUNCTION),
            ("/p/c.js", CLASS),
        ],
    );
    assert_eq!(out.bind["/p/a.js"], vec![1101]);
    assert_eq!(out.bind["/p/b.ts"], vec![1101]);
    // Only the `with` inside the strict function or class; the top-level one is sloppy.
    assert_eq!(out.bind["/p/f.js"], vec![1101]);
    assert_eq!(out.bind["/p/c.js"], vec![1101]);
    for name in ["/out/a.js", "/out/b.js"] {
        assert!(out.emitted[name].starts_with("\"use strict\";"), "{}", out.emitted[name]);
    }
    assert!(!has_use_strict(&out.emitted["/out/c.js"]));
}

#[test]
fn default_options_stay_strict() {
    for always_strict in [Tristate::Unknown, Tristate::True] {
        let out = compile(always_strict, &[("/p/a.js", SLOPPY), ("/p/b.ts", SLOPPY)]);
        for name in ["/p/a.js", "/p/b.ts"] {
            assert_eq!(out.bind[name], vec![1101, 1212, 1102, 1100], "{name}");
        }
        for text in out.emitted.values() {
            assert!(text.starts_with("\"use strict\";"), "{text}");
        }
    }
}
