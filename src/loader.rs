use std::{cell::RefCell, collections::HashMap, env, fs, path::{Path, PathBuf}, rc::Rc};

use crate::{
    interpreter::{interpret, ModuleContext, ProgramContext, RuntimeError}, lexer, loader, parser::{self, Program, Statement, ValueHolder, VariableRef}, translator
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
    pub statements: Vec<Statement>,
    pub imports: Vec<Import>,
    pub program: Rc<RefCell<ProgramContext>>
}

struct PathResovler {
    process_path: PathBuf,
    current_path: PathBuf
}

impl PathResovler {
    pub fn resolve(&self, path: PathBuf) -> Result<PathBuf, RuntimeError> {
        let base_path = {
            if path.starts_with("./") || path.starts_with(".\\") {
                &self.current_path
            } else {
                &self.process_path
            }
        };

        let process_path = self.process_path.canonicalize().map_err(|_| RuntimeError::Custom(format!("Invalid path: {:?}", self.process_path)))?;

        let full_path = base_path.join(path);
        let full_path = full_path.canonicalize().map_err(|_| RuntimeError::Custom(format!("Invalid path: {:?}", full_path)))?;

        if !full_path.starts_with(process_path) {
            return Err(RuntimeError::Custom("Tried to access file outside program scope".into()))
        }

        Ok(full_path)
    }
}

impl Module {
    pub fn new(source: &str, program: Rc<RefCell<ProgramContext>>) -> Self {
        Self {
            source: source.into(),
            exports: HashMap::new(),
            context: None,
            statements: vec![],
            imports: vec![],
            program
        }
    }

    pub fn resolve_path(path: &str) -> Result<String, RuntimeError> {
        Self::resolve_path_internal(path.into(), None)
    }

    fn resolve_path_internal(path: PathBuf, current_path: Option<PathBuf>) -> Result<String, RuntimeError> {
        let current_dir = env::current_dir().unwrap();

        let resolver = PathResovler {
            process_path: current_dir.clone(),
            current_path: if current_path.is_some() { current_path.unwrap() } else { current_dir },
        };

        resolver.resolve(PathBuf::from(path)).map(|buf| buf.to_str().unwrap().to_string())
    }

    pub fn is_loaded(&self) -> bool {
        return self.context.is_some();
    }

    fn scan(module: Rc<RefCell<Module>>) -> Result<(), RuntimeError> {
        let source = module.borrow().source.clone();
        

        let contents = fs::read_to_string(&source)
            .map_err(|_| RuntimeError::Custom(format!("Module not found : {}", source)))?;

        let tokens = lexer::lex(contents);

        let name = Path::new(&source)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let _ = fs::write(
            format!("debug/{}-tokens.json", name),
            serde_json::to_string_pretty(&tokens).unwrap(),
        );

        let ast = parser::parse(tokens);
        let _ = fs::write(
            format!("debug/{}-ast.json", name),
            serde_json::to_string_pretty(&ast).unwrap(),
        );

        let ir = translator::translate(ast);
        let _ = fs::write(
            format!("debug/{}-ir.json", name),
            serde_json::to_string_pretty(&ir).unwrap(),
        );

        let statements: Vec<Statement> = ir
            .body
            .iter()
            .map(|statement| match statement {
                Statement::ImportIR { specifiers, source } => {
                    module.borrow_mut().imports.push(Import {
                        specifiers: specifiers.clone(),
                        source: source.into(),
                    });
                    Ok(None) // We skip adding to statements
                }

                Statement::Export { declaration } => {
                    let var_ref = match declaration.clone() {
                        box Statement::FunctionIR { var_ref, .. } => var_ref,
                        _ => panic!(),
                    };

                    module.borrow_mut().exports.insert(
                        var_ref.name.clone().unwrap(),
                        ValueHolder::LazyRef {
                            slot: var_ref.slot,
                            module: source.clone(),
                        },
                    );

                    Ok(Some(*declaration.clone()))
                }

                statement => Ok(Some(statement.clone())),
            })
            // Now we have Iterator<Item = Result<Option<Statement>, RuntimeError>>
            // Flatten it into Result<Vec<Statement>, RuntimeError>
            .collect::<Result<Vec<_>, _>>() // Collect Vec<Option<Statement>>
            .map(|v| v.into_iter().flatten().collect())?;

        {
            let imports = module.borrow().imports.clone();
            for import in imports {
                let program = module.borrow().program.clone();

                let source = Module::resolve_path_internal(import.source.clone().into(), Some(PathBuf::from(module.borrow().source.clone()).parent().unwrap().to_path_buf()))?;

                let module_ref = resolve_module(&source, program)?;
                let imported_mod = module_ref.borrow();
                for specifier in import.specifiers {
                    let name = &specifier.name.clone().unwrap();
                    if !imported_mod.exports.contains_key(name) {
                        return Err(RuntimeError::NoSuchProperty(format!(
                            "{} does not expose {name}", import.source
                        )));
                    }
                }
            }
        }

        module.borrow_mut().statements = statements;

        let _ = fs::write(
            format!("debug/{}-statements.json", name),
            serde_json::to_string_pretty(&module.borrow().statements).unwrap(),
        );

        Ok(())
    }

    pub fn execute(module_ref: Rc<RefCell<Module>>) -> Result<(), RuntimeError> {
        if module_ref.borrow().is_loaded() {
            panic!("Module is already loaded")
        }

        let context = ModuleContext::new(module_ref.borrow().program.clone());

        let context_ref = Rc::new(RefCell::new(context));

        context_ref.borrow_mut().environment.init(context_ref.clone());

        module_ref.borrow_mut().context = Some(context_ref);

        {
            let context = module_ref.borrow().context.clone().unwrap();
            let mut context = context.borrow_mut();

            let imports = module_ref.borrow().imports.clone();
            for import in imports {
                for specifier in import.specifiers {
                    let name = specifier.clone().name.unwrap();
                    let module = module_ref.borrow();
                    let program = module.program.borrow();

                    let source = Module::resolve_path_internal(import.source.clone().into(), Some(PathBuf::from(module.source.clone()).parent().unwrap().to_path_buf()))?;
                    
                    let module = program.modules.get(&source).unwrap().borrow();
                    let value = module
                        .exports
                        .get(&name)
                        .ok_or(RuntimeError::VariableNotFound(format!(
                            "Module has no such property: {}",
                            name
                        )))?;

                    context
                        .environment
                        .set(specifier.clone(), value.clone(), true)?;
                }
            }
        }

        let statements = module_ref.borrow().statements.clone();

        interpret(
            Program {
                body: statements,
            },
            module_ref.borrow().context.clone().unwrap(),
        )?;

        Ok(())
    }
}

pub fn resolve_module(path: &str, pg_context: Rc<RefCell<ProgramContext>>) -> Result<Rc<RefCell<Module>>, RuntimeError> {
    if !pg_context.borrow().modules.contains_key(path) {
        let module = Rc::new(RefCell::new(Module::new(path, pg_context.clone())));

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

pub fn run_main(path: &str) -> Result<(), RuntimeError> {
    let pg_context = Rc::new(RefCell::new(ProgramContext::new()));

    let path = Module::resolve_path(path)?;

    let module = loader::resolve_module(&path, pg_context.clone())?;

    Module::execute(module)?;

    Ok(())
}