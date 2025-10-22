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

    fn scan(&mut self) -> Result<(), RuntimeError> {
        let contents = fs::read_to_string(&self.source)
            .map_err(|_| RuntimeError::Custom("Module not found".into()))?;

        let tokens = lexer::lex(contents);

        let name = Path::new(&self.source)
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
                    let module_ref = resolve_module(source, self.program.clone())?;
                    let imported_mod = module_ref.borrow();
                    for specifier in specifiers {
                        let name = &specifier.name.clone().unwrap();
                        if !imported_mod.exports.contains_key(name) {
                            return Err(RuntimeError::NoSuchProperty(format!(
                                "{source} does not expose {name}"
                            )));
                        }
                    }
                    self.imports.push(Import {
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

                    self.exports.insert(
                        var_ref.name.clone().unwrap(),
                        ValueHolder::LazyRef {
                            slot: var_ref.slot,
                            module: self.source.clone(),
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

        self.statements = statements;

        let _ = fs::write(
            format!("debug/{}-statements.json", name),
            serde_json::to_string_pretty(&self.statements).unwrap(),
        );

        println!("Importing mdo");

        Ok(())
    }

    pub fn execute(&mut self) -> Result<(), RuntimeError> {
        if self.is_loaded() {
            panic!("Module is already loaded")
        }

        let context = ModuleContext::new(self.program.clone());

        self.context = RefCell::new(Some(context));

        let mut context_ref = self.context.borrow_mut();
        let mut context = context_ref.as_mut().unwrap();

        for import in self.imports.clone() {
            for specifier in import.specifiers {
                println!("{:?}", specifier);
                let name = specifier.clone().name.unwrap();
                let program = self.program.borrow();
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
                body: self.statements.clone(),
            },
            &mut context,
        )?;

        Ok(())
    }
}

pub fn resolve_module(path: &str, pg_context: Rc<RefCell<ProgramContext>>) -> Result<Rc<RefCell<Module>>, RuntimeError> {
    if !pg_context.borrow().modules.contains_key(path) {
        let mut module = Module::new(path, pg_context.clone());
        module.scan()?;
        pg_context.borrow_mut().modules.insert(path.into(), module.into());
    }

    let module = Rc::new(pg_context.borrow().modules.get(path).unwrap().clone());

    Ok(module)
}

pub fn run_main(path: &str) -> Result<(), RuntimeError> {
    let pg_context = Rc::new(RefCell::new(ProgramContext::new()));

    let module = loader::resolve_module(path, pg_context.clone())?;

    module.borrow_mut().execute()?;

    Ok(())
}