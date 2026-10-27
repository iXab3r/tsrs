// the worker pool's thread count and stack size come from code, and compiler work (including the
// include-glob directory walk) never starts rayon's implicit global pool. One test per process: the pool is global.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

use tsrs_compiler::{
    Context, ProgramOptions, WorkerPoolOptions, WorkerPoolStarted, configure_worker_pool,
    new_compiler_host, new_program, worker_pool,
};
use tsrs_core::Tristate;
use tsrs_tsoptions::ParseConfigHost;
use tsrs_vfs::{FS, vfstest};

struct ConfigHost(Arc<dyn FS>);
impl ParseConfigHost for ConfigHost {
    fn fs(&self) -> &dyn FS {
        &*self.0
    }
    fn get_current_directory(&self) -> &'static str {
        "/p"
    }
}

#[test]
fn pool_shape_comes_from_code_and_the_global_pool_stays_unused() {
    configure_worker_pool(WorkerPoolOptions {
        threads: NonZeroUsize::new(3),
        stack_size: Some(8 << 20),
    })
    .unwrap();

    let mut files = vec![(
        "/p/tsconfig.json".to_string(),
        r#"{"compilerOptions":{"noLib":true,"strict":true},"include":["src/**/*.ts"]}"#.to_string(),
    )];
    for dir in 0..4 {
        for file in 0..5 {
            files.push((
                format!("/p/src/d{dir}/f{file}.ts"),
                format!("export const v{dir}_{file}: number = {file};\n"),
            ));
        }
    }
    let fs: Arc<dyn FS> = Arc::new(vfstest::from_map(files, true));
    let config_host = ConfigHost(Arc::clone(&fs));
    let (config, errors) = tsrs_tsoptions::get_parsed_command_line_of_config_file(
        "/p/tsconfig.json",
        None,
        None,
        &config_host,
        None,
    );
    assert_eq!(errors.len(), 0);
    let config = config.unwrap();
    assert_eq!(config.file_names().len(), 20, "the include glob walked every directory");
    let mut opts = ProgramOptions::new(
        tsrs_core::P::new(config),
        new_compiler_host("/p", fs, "", None, None, None),
    );
    opts.single_threaded = Tristate::False;
    let program = new_program(opts);
    assert_eq!(program.get_semantic_diagnostics(&Context::background(), None).len(), 0);

    let names: BTreeSet<String> = worker_pool()
        .broadcast(|_| std::thread::current().name().unwrap_or_default().to_string())
        .into_iter()
        .collect();
    assert_eq!(names, ["tsrs-worker-0", "tsrs-worker-1", "tsrs-worker-2"].map(String::from).into());
    assert_eq!(configure_worker_pool(WorkerPoolOptions::default()), Err(WorkerPoolStarted));
    // build_global fails once rayon's global pool exists.
    assert!(
        rayon::ThreadPoolBuilder::new().num_threads(1).build_global().is_ok(),
        "compiler work started rayon's global pool"
    );
}
