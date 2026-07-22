use std::{cell::RefCell, collections::HashMap, env, fs, path::{Path, PathBuf}, rc::Rc, sync::Arc};

use parking_lot::Mutex;
use ron::ser::PrettyConfig;

use crate::{
    compiler::{self, CompileInfo}, errors::{ErrorSource, LanguageError, LanguageErrorKind, LanguageResult, provide_source}, interpreter::{ModuleContext, ProgramContext}, lexer, loader, new_loader::EntryPoint, new_translator::{self}, parser::{self, IRStatement, IRStatementKind, ValueHolder, VariableRef}, stdlib::CoreModules
};

#[derive(Debug, Clone)]
pub struct ModuleSource {
    pub path: String,
    pub kind: ModuleKind
}

pub fn resolve_module(path: &str, current_folder: Option<PathBuf>) -> LanguageResult<ModuleSource> {
    let current_dir = env::current_dir().unwrap();

    let resolver = PathResolver {
        process_path: current_dir.clone(),
        current_path: if current_folder.is_some() { current_folder.unwrap() } else { current_dir },
    };

    let resolved = match resolver.resolve(PathBuf::from(path)).map(|buf| buf.to_str().unwrap().to_string()) {
        Ok(resolved) => resolved,
        Err(_) => path.to_string()
    };

    let kind = if PathBuf::from(resolved.clone()).exists() {
        ModuleKind::Standard
    } else if resolved.starts_with("core:") {
        ModuleKind::Core
    } else {
        return Err(LanguageError::from(LoaderError::ModuleNotFound(resolved.to_string())))
    };

    Ok(ModuleSource {
        path: resolved,
        kind
    })
}

pub fn parse_module_source(source: &ModuleSource, pg_context: Rc<RefCell<ProgramContext>>) -> LanguageResult<Arc<Mutex<Module>>> {
    if !pg_context.borrow().modules.contains_key(&source.path) {
        let module = Arc::new(Mutex::new(Module::new(source.clone(), pg_context.clone())));

        // Borrow only to insert, then drop it immediately.
        {
            let modules = &mut pg_context.borrow_mut().modules;
            modules.insert(source.path.clone(), module.clone());
        }

        Module::load(module)?;
    }

    let module = pg_context.borrow().modules.get(&source.path).unwrap().clone();

    Ok(module)
}

pub fn read_module(source: &ModuleSource) -> LanguageResult<String> {
    let path = source.path.clone();

    let contents = match source.kind {
        ModuleKind::Core => {
            let embedded_file = CoreModules::get(&format!("{}.nlf", path.strip_prefix("core:").unwrap()))
                .ok_or_else(|| LanguageError::from(LoaderError::ModuleNotFound(path.clone())))?;

            String::from_utf8(embedded_file.data.into_owned()).map_err(|_| LanguageError::from(LoaderError::TODO))?
        }
        ModuleKind::Standard => {
            fs::read_to_string(&path)
                // TODO: proper start / and
                .map_err(|_| LanguageError::from(LoaderError::ModuleNotFound(path.clone())))?
        }
        ModuleKind::Library => {
            todo!()
        }
    };

    Ok(contents)
}

#[derive(Debug, Clone)]
pub struct Import {
    pub source: String,
    pub specifiers: Vec<VariableRef>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub source: ModuleSource,
    pub exports: HashMap<String, ValueHolder>,
    pub context: Option<Rc<RefCell<ModuleContext>>>,
    pub statements: Rc<[IRStatement]>,
    pub imports: Vec<Import>,
    pub program: Rc<RefCell<ProgramContext>>,
    pub contents: Option<Arc<Mutex<String>>>,
}

pub struct PathResolver {
    pub process_path: PathBuf,
    pub current_path: PathBuf
}

#[derive(Debug)]
pub enum LoaderError {
    ModuleNotFound(String),
    ImportNotFound(String),
    BoundsViolation(String),
    TODO
}

impl LanguageErrorKind for LoaderError {}

impl PathResolver {
    pub fn resolve(&self, path: PathBuf) -> LanguageResult<PathBuf> {
        let base_path = {
            if path.starts_with("./") || path.starts_with(".\\") {
                &self.current_path
            } else {
                &self.process_path
            }
        };

        let process_path = self.process_path.canonicalize().map_err(|_| LanguageError::from(LoaderError::ModuleNotFound(self.process_path.to_str().unwrap().to_string())))?;

        let full_path = base_path.join(path);
        let full_path = full_path.canonicalize().map_err(|_| LanguageError::from(LoaderError::ModuleNotFound(full_path.to_str().unwrap().to_string())))?;

        if !full_path.starts_with(process_path) {
            return Err(LanguageError::from(LoaderError::BoundsViolation(full_path.to_str().unwrap().to_string())))
        }

        Ok(full_path)
    }
}

#[derive(Debug, Clone)]
pub enum ModuleKind {
    Standard,
    Core,
    Library
}

impl Module {
    pub fn new(resolved_file: ModuleSource, program: Rc<RefCell<ProgramContext>>) -> Self {
        Self {
            source: resolved_file,
            exports: HashMap::new(),
            context: None,
            statements: Rc::new([]),
            imports: vec![],
            program,
            contents: None
        }
    }

    pub fn is_loaded(&self) -> bool {
        return self.context.is_some();
    }

    fn _load(module: Arc<Mutex<Module>>, contents: &String) -> LanguageResult<()> {
        let path = module.lock().source.path.clone();

        let tokens = lexer::lex(contents.clone())?;
        
        let name = Path::new(&path)
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

        let ir = new_translator::translate(ast)?;
        let _ = fs::write(
            format!("core/debug/{}-ir.ron", name),
            ron::ser::to_string_pretty(&ir.program, PrettyConfig::default()).unwrap(),
        );

        let build = compiler::compile_program(&ir, CompileInfo {
            function_infos: ir.function_infos.clone(),
            class_infos: ir.class_infos.clone(),
            ..CompileInfo::no_resolver()
        });
        
        let _entry_point = EntryPoint::new(&ir, build);

        let statements: Rc<[IRStatement]> = ir
            .program
            .body
            .iter()
            .map(|statement| match &statement.kind {
                IRStatementKind::Import { specifiers, source } => {
                    module.lock().imports.push(Import {
                        specifiers: specifiers.clone(),
                        source: source.value.clone()
                    });
                    Ok(None) // We skip adding to statements
                }
                IRStatementKind::Export { declaration } => {
                    let var_ref = match declaration.kind.clone() {
                        IRStatementKind::Function { variable, .. } => variable,
                        IRStatementKind::Class { variable, .. } => variable,
                        _ => panic!(),
                    };

                    module.lock().exports.insert(
                        var_ref.name.clone().unwrap(),
                        ValueHolder::LazyRef {
                            slot: var_ref.slot,
                            module: path.clone(),
                        },
                    );

                    Ok(Some(*declaration.clone()))
                }
                kind => {
                    let mut statement = statement.clone();
                    statement.kind = kind.clone();
                    Ok(Some(statement.clone()))
                },
            })
            // Now we have Iterator<Item = Result<Option<Statement>, RuntimeError>>
            // Flatten it into Result<Vec<Statement>, RuntimeError>
            .collect::<Result<Vec<_>, _>>() // Collect Vec<Option<Statement>>
            .map(|v| v.into_iter().flatten().collect())?;

        {
            let imports = module.lock().imports.clone();
            for import in imports {
                let program = module.lock().program.clone();

                let resolved_module = loader::resolve_module(&import.source, Some(PathBuf::from(module.lock().source.path.clone()).parent().unwrap().to_path_buf()))?;

                let module_ref = parse_module_source(&resolved_module, program)?;
                let imported_mod = module_ref.lock();
                for specifier in import.specifiers {
                    let name = &specifier.name.clone().unwrap();
                    if !imported_mod.exports.contains_key(name) { 
                        // TODO: Correct source bindings
                        // RuntimeError::NoSuchProperty(format!(
                        //     "{} does not expose {name}", import.source
                        // ))
                        return Err(LanguageError::with_source(LoaderError::ImportNotFound(import.source), 0, 0));
                    }
                }
            }
        }

        module.lock().statements = statements;
        module.lock().contents = Some(Arc::new(Mutex::new(contents.clone())));

        let _ = fs::write(
            format!("core/debug/{}-statements.ron", name),
            ron::ser::to_string_pretty(&module.lock().statements, PrettyConfig::default()).unwrap(),
        );

        Ok(())
    }

    fn load(module: Arc<Mutex<Module>>) -> LanguageResult<()> {
        let source = module.lock().source.clone();

        let contents = read_module(&source)?;
        
        let mut result = Self::_load(module, &contents);

        provide_source(&mut result, ErrorSource {
            path: source.path,
            contents: Arc::new(Mutex::new(contents))
        });

        result
    }

    fn _execute(module_ref: Arc<Mutex<Module>>) -> LanguageResult<()> {
        if module_ref.lock().is_loaded() {
            panic!("Module is already loaded")
        }

        let context_ref = ModuleContext::new(module_ref.lock().program.clone());

        context_ref.borrow_mut().environment.init();

        module_ref.lock().context = Some(context_ref);

        {
            let context = module_ref.lock().context.clone().unwrap();
            let mut context = context.borrow_mut();

            let imports = module_ref.lock().imports.clone();
            for import in imports {
                for specifier in &import.specifiers {
                    let name = specifier.name.clone().unwrap();
                    let module = module_ref.lock();
                    let program = module.program.borrow();

                    let source = loader::resolve_module(&import.source, Some(PathBuf::from(module.source.path.clone()).parent().unwrap().to_path_buf()))?;
                    
                    let module = program.modules.get(&source.path).unwrap().lock();
                    let value = module
                        .exports
                        .get(&name)
                        .ok_or_else(|| LanguageError::with_source(LoaderError::ImportNotFound(name.to_string()), 0, 0))?;
                    
                    context
                        .environment
                        .set(specifier, value.clone(), true).map_err(|_| LanguageError::with_source(LoaderError::TODO, 0, 0))?;
                }
            }
        }

        let _statements = module_ref.lock().statements.clone();

        // vm::run_main(program, build)

        // interpret(
        //     Program {
        //         body: statements,
        //     },
        //     &mut module_ref.lock().context.clone().unwrap().borrow_mut(),
        // )?;

        // println!("{end:.2?}");
        
        Ok(())
    }

    pub fn execute(module_ref: Arc<Mutex<Module>>) -> LanguageResult<()> {
        let mut result = Self::_execute(module_ref.clone());

        let module = module_ref.lock();

        if let Some(contents) = &module.contents {
            provide_source(&mut result, ErrorSource {
                path: module.source.path.clone(),
                contents: contents.clone()
            });
        }

        result
    }
}

pub fn run_main(path: &str) -> LanguageResult<()> {
    let pg_context = Rc::new(RefCell::new(ProgramContext::new()));

    let module_source = loader::resolve_module(path, None)?;

    let module = loader::parse_module_source(&module_source, pg_context.clone())?;

    Module::execute(module)?;

    Ok(())
}