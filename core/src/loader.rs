use std::{cell::RefCell, collections::HashMap, env, fs, path::{Path, PathBuf}, rc::Rc, str::FromStr, time::Instant};

use ron::ser::PrettyConfig;

use crate::{compiler::{self, CompileInfo, CompilerBuild, resolvers::{FileSystemModuleResolver, ModuleResolver}}, errors::LanguageResult, lexer, translator::{self, TranslatorOutput}, parser, vm::{self, ExecutionInfo, VM, create_vm}};

#[derive(Debug, Clone)]
pub enum ModuleKind {
    Standard,
    Core,
    Library
}

#[derive(Debug, Clone)]
pub struct ModuleSource {
    pub path: String,
    pub kind: ModuleKind
}

pub struct EntryPoint {
    pub module: vm::ModuleData,
    pub execution_info: ExecutionInfo
}

impl EntryPoint {
    pub fn new(ir: &TranslatorOutput, build: CompilerBuild) -> Self {
        EntryPoint {
            module: vm::ModuleData {
                name: "main".to_string(),
                code: build.operations,
                constants: build.consts,
                strings: build.strings,
                functions: build.functions,
                exports: build.exports,
                imports: build.imports
            },
            execution_info: ExecutionInfo {
                local_count: ir.local_count
            }
        }
    }
}

pub struct Module {
    pub source: ModuleSource,
    pub ir: TranslatorOutput,
    pub build: CompilerBuild,
}

pub struct Loader {
    pub(crate) vm: Rc<RefCell<VM>>,
    pub(crate) modules: HashMap<String, Module>,
    pub(crate) resolver: Rc<dyn ModuleResolver>
}

impl Loader {
    pub fn new(entry_point: String, resolver: Rc<dyn ModuleResolver>) -> LoaderRef {
        let source = resolver.resolve_source(&entry_point, None).unwrap();

        let module = Self::resolve_module(source, &resolver).unwrap();

        let mut modules = HashMap::new();

        let entry_point = EntryPoint::new(&module.ir, module.build.clone());

        modules.insert(module.source.path.clone(), module);

        let loader = Rc::new_cyclic(|loader| {
            let vm = create_vm(entry_point, loader.clone());
            
            RefCell::new(Self { vm: Rc::new(RefCell::new(vm)), modules, resolver })
        });

        LoaderRef(loader)
    }

    pub fn get_or_load(&mut self, name: &str) -> &mut Module {
        self.modules.entry(name.to_string()).or_insert_with(|| {
            let source = self.resolver.resolve_source(&name.to_string(), None).unwrap();

            Self::resolve_module(source, &self.resolver).unwrap()
        })
    }

    pub fn resolve_module(source: ModuleSource, resolver: &Rc<dyn ModuleResolver>) -> LanguageResult<Module> {
        let contents = resolver.read(&source)?;

        let tokens = lexer::lex(contents.clone())?;
        
        let name = Path::new(&source.path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        
        let _ = fs::write(
            format!("core/debug/{}-tokens.ron", name),
            ron::ser::to_string_pretty(&tokens, PrettyConfig::default()).unwrap(),
        );
        
        let ast = parser::parse(tokens)?;

        let _ = fs::write(
            format!("core/debug/{}-ast.ron", name),
            ron::ser::to_string_pretty(&ast, PrettyConfig::default()).unwrap(),
        );

        let ir = translator::translate(ast)?;
        let _ = fs::write(
            format!("core/debug/{}-ir.ron", name),
            ron::ser::to_string_pretty(&ir.program, PrettyConfig::default()).unwrap(),
        );

        let file_path: PathBuf = source.path.clone().into();

        let build = compiler::compile_program(&ir, CompileInfo {
            source: file_path.parent().unwrap().into(),
            function_infos: ir.function_infos.clone(),
            class_infos: ir.class_infos.clone(),
            path_resolver: resolver.clone()
        });

        let _ = fs::write(format!("core/debug/vm/build-{}.dbg", PathBuf::from_str(&source.path).unwrap().file_name().unwrap().to_str().unwrap()), build.operations.iter().map(|op| format!("{op:?}")).collect::<Vec<String>>().join("\n"));

        Ok(Module { source, ir, build })
    }
    
    pub fn resolve_name(source: &str) -> String {
        let resolver: Rc<dyn ModuleResolver> = Rc::new(FileSystemModuleResolver {
            process_path: env::current_dir().unwrap()
        });

        resolver.resolve_source(source, None).unwrap().path
    }
}

pub struct LoaderRef(pub Rc<RefCell<Loader>>);

impl LoaderRef {
    pub fn run_main(&self) {
        let loader = self.0.borrow();
        let vm = loader.vm.clone();
        
        drop(loader);
    
        let start = Instant::now();
        vm::run(&mut vm.borrow_mut());
        println!("{:2?}", start.elapsed())
    }
}