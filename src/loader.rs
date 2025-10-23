use std::{cell::{RefCell}, collections::HashMap, fs, path::Path, rc::Rc};

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
    pub context: RefCell<Option<ModuleContext>>,
    pub statements: Vec<Statement>,
    pub imports: Vec<Import>,
    pub program: Rc<RefCell<ProgramContext>>
}

impl Module {
    pub fn new(source: &str, program: Rc<RefCell<ProgramContext>>) -> Self {
        Self {
            source: source.into(),
            exports: HashMap::new(),
            context: RefCell::new(None),
            statements: vec![],
            imports: vec![],
            program
        }
    }

    pub fn is_loaded(&self) -> bool {
        return self.context.borrow().is_some();
    }

    fn scan(module: Rc<RefCell<Module>>) -> Result<(), RuntimeError> {
        let source = module.borrow().source.clone();

        let contents = fs::read_to_string(&source)
            .map_err(|_| RuntimeError::Custom("Module not found".into()))?;

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
                let module_ref = resolve_module(&import.source, program)?;
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

    pub fn execute(module: Rc<RefCell<Module>>) -> Result<(), RuntimeError> {
        if module.borrow().is_loaded() {
            panic!("Module is already loaded")
        }

        let context = ModuleContext::new(module.borrow().program.clone());

        module.borrow_mut().context = RefCell::new(Some(context));

        let borrowed_module = module.borrow();
        let mut context_ref = borrowed_module.context.borrow_mut();
        let mut context = context_ref.as_mut().unwrap();

        let imports = module.borrow().imports.clone();
        for import in imports {
            for specifier in import.specifiers {
                let name = specifier.clone().name.unwrap();
                let module = module.borrow();
                let program = module.program.borrow();
                let module = program.modules.get(&import.source).unwrap().borrow();
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

        interpret(
            Program {
                body: module.borrow().statements.clone(),
            },
            &mut context,
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

    let module = loader::resolve_module(path, pg_context.clone())?;

    Module::execute(module)?;

    Ok(())
}