use std::{cell::RefCell, collections::HashMap, env, fs, path::{Path, PathBuf}, rc::Rc, sync::{Arc}};

use parking_lot::Mutex;

use crate::{
    errors::{ErrorSource, LanguageError, LanguageErrorTrait, LanguageResult, provide_source}, interpreter::{ModuleContext, ProgramContext, interpret}, lexer, loader, parser, parser::{Program, Statement, StatementKind, ValueHolder, VariableRef}, stdlib::CoreModules, translator
};

#[derive(Debug, Clone)]
pub struct Import {
    pub source: String,
    pub specifiers: Vec<VariableRef>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub source: String,
    pub exports: HashMap<String, ValueHolder>,
    pub context: Option<Rc<RefCell<ModuleContext>>>,
    pub statements: Rc<[Statement]>,
    pub imports: Vec<Import>,
    pub program: Rc<RefCell<ProgramContext>>,
    pub contents: Option<Arc<Mutex<String>>>,
    pub kind: ModuleKind
}

struct PathResovler {
    process_path: PathBuf,
    current_path: PathBuf
}

#[derive(Debug)]
enum LoaderError {
    ModuleNotFound(String),
    ImportNotFound(String),
    BoundsViolation(String),
    TODO
}

impl LanguageErrorTrait for LoaderError {}

impl PathResovler {
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
    pub fn new(source: &str, kind: ModuleKind, program: Rc<RefCell<ProgramContext>>) -> Self {
        Self {
            source: source.into(),
            exports: HashMap::new(),
            context: None,
            statements: Rc::new([]),
            imports: vec![],
            program,
            contents: None,
            kind
        }
    }

    pub fn resolve_path(path: &str) -> LanguageResult<String> {
        Self::resolve_path_internal(path.into(), None)
    }

    fn resolve_path_internal(path: PathBuf, current_path: Option<PathBuf>) -> LanguageResult<String> {
        let current_dir = env::current_dir().unwrap();

        let resolver = PathResovler {
            process_path: current_dir.clone(),
            current_path: if current_path.is_some() { current_path.unwrap() } else { current_dir },
        };

        match resolver.resolve(PathBuf::from(path.clone())).map(|buf| buf.to_str().unwrap().to_string()) {
            Ok(resolved) => Ok(resolved),
            Err(_) => Ok(path.to_str().unwrap().to_string())
        }
    }

    pub fn is_loaded(&self) -> bool {
        return self.context.is_some();
    }

    fn _scan(module: Arc<Mutex<Module>>, contents: &String) -> LanguageResult<()> {
        let source = module.lock().source.clone();

        let tokens = lexer::lex(contents.clone())?;
        
        let name = Path::new(&source)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let _ = fs::write(
            format!("core/debug/{}-tokens.json", name),
            serde_json::to_string_pretty(&tokens).unwrap(),
        );

        let ast = parser::parse(tokens)?;

        let _ = fs::write(
            format!("core/debug/{}-ast.json", name),
            serde_json::to_string_pretty(&ast).unwrap(),
        );

        let ir = translator::translate(ast)?;
        let _ = fs::write(
            format!("core/debug/{}-ir.json", name),
            serde_json::to_string_pretty(&ir).unwrap(),
        );

        let statements: Rc<[Statement]> = ir
            .body
            .iter()
            .map(|statement| match &statement.kind {
                StatementKind::ImportIR { specifiers, source } => {
                    module.lock().imports.push(Import {
                        specifiers: specifiers.clone(),
                        source: source.value.clone()
                    });
                    Ok(None) // We skip adding to statements
                }

                StatementKind::Export { declaration } => {
                    let var_ref = match declaration.kind.clone() {
                        StatementKind::FunctionIR { var_ref, .. } => var_ref,
                        StatementKind::ClassIR { var_ref, .. } => var_ref,
                        _ => panic!(),
                    };

                    module.lock().exports.insert(
                        var_ref.name.clone().unwrap(),
                        ValueHolder::LazyRef {
                            slot: var_ref.slot,
                            module: source.clone(),
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

                let source = Self::resolve_path_internal(import.source.clone().into(), Some(PathBuf::from(module.lock().source.clone()).parent().unwrap().to_path_buf()))?;

                let module_ref = resolve_module(&source, program)?;
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
            format!("core/debug/{}-statements.json", name),
            serde_json::to_string_pretty(&module.lock().statements).unwrap(),
        );

        Ok(())
    }

    fn scan(module: Arc<Mutex<Module>>) -> LanguageResult<()> {
        let source = module.lock().source.clone();

        let contents = match module.lock().kind {
            ModuleKind::Core => {
                let embedded_file = CoreModules::get(&format!("{}.nlf", source.strip_prefix("core:").unwrap()))
                    .ok_or_else(|| LanguageError::from(LoaderError::ModuleNotFound(source.clone())))?;

                String::from_utf8(embedded_file.data.into_owned()).map_err(|_| LanguageError::from(LoaderError::TODO))?
            }
            ModuleKind::Standard => {
                fs::read_to_string(&source)
                    // TODO: proper start / and
                    .map_err(|_| LanguageError::from(LoaderError::ModuleNotFound(source.clone())))?
            }
            ModuleKind::Library => {
                todo!()
            }
        };
        
        let mut result = Self::_scan(module, &contents);

        provide_source(&mut result, ErrorSource {
            path: source,
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

                    let source = Self::resolve_path_internal(import.source.clone().into(), Some(PathBuf::from(module.source.clone()).parent().unwrap().to_path_buf()))?;
                    
                    let module = program.modules.get(&source).unwrap().lock();
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

        let statements = module_ref.lock().statements.clone();

        interpret(
            Program {
                body: statements,
            },
            &mut module_ref.lock().context.clone().unwrap().borrow_mut(),
        )?;

        Ok(())
    }

    pub fn execute(module_ref: Arc<Mutex<Module>>) -> LanguageResult<()> {
        let mut result = Self::_execute(module_ref.clone());

        let module = module_ref.lock();

        if let Some(contents) = &module.contents {
            provide_source(&mut result, ErrorSource {
                path: module.source.clone(),
                contents: contents.clone()
            });
        }

        result
    }
}

pub fn resolve_module(path: &str, pg_context: Rc<RefCell<ProgramContext>>) -> LanguageResult<Arc<Mutex<Module>>> {
    let kind = if PathBuf::from(path).exists() {
        ModuleKind::Standard
    } else if path.starts_with("core:") {
        ModuleKind::Core
    } else {
        return Err(LanguageError::from(LoaderError::ModuleNotFound(path.to_string())))
    };

    if !pg_context.borrow().modules.contains_key(path) {
        let module = Arc::new(Mutex::new(Module::new(&path, kind, pg_context.clone())));

        // Borrow only to insert, then drop it immediately.
        {
            let modules = &mut pg_context.borrow_mut().modules;
            modules.insert(path.into(), module.clone());
        }

        Module::scan(module)?;
    }

    let module = pg_context.borrow().modules.get(path).unwrap().clone();

    Ok(module)
}

pub fn run_main(path: &str) -> LanguageResult<()> {
    let pg_context = Rc::new(RefCell::new(ProgramContext::new()));

    let path = Module::resolve_path(path)?;

    let module = loader::resolve_module(&path, pg_context.clone())?;

    Module::execute(module)?;

    Ok(())
}