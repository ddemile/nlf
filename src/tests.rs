use clap::Error;
use inline_colorization::*;
use std::{
    cell::RefCell, collections::HashMap, fmt::format, fs, panic::{self, catch_unwind}, path::{Path, PathBuf}, rc::Rc, time::{Duration, Instant}
};
use rayon::prelude::*;

use crate::{errors::ErrorSource, interpreter::{self, ModuleContext, ProgramContext, RuntimeError}, lexer, parser, translator};

struct Test{
    pub name: String,
    pub content: String,
    pub file: String
}

struct TestOutput {
    pub name: String,
    pub result: Result<Duration, String>
}

pub fn run_tests(base_dir: &Path) {
    let paths = fs::read_dir(base_dir).unwrap();

    // Save the original hook
    let default_hook = panic::take_hook();

    // Replace it with a no-op hook to suppress printing
    panic::set_hook(Box::new(|_| {}));

    let mut tests: Vec<Test> = vec![];

    for entry in paths {
        let path = entry.unwrap().path();
        let contents = fs::read_to_string(&path).unwrap();
        let lines = contents.lines().collect::<Vec<&str>>();

        let mut test_name: Option<String> = None;
        let mut test_content = String::new();

        let file_path = format!(
            "{}/{}",
            base_dir.display(),
            path.file_name().unwrap().to_str().unwrap()
        );

        for line in lines {
            if test_name.is_some() {
                test_content.push_str(format!("{}\n", line).as_str());
            }

            if line.starts_with("// Name: ") {
                if test_name.is_some() {
                    tests.push(Test {
                        name: test_name.unwrap(),
                        content: test_content,
                        file: file_path.clone()
                    });
                }
                test_content = String::new();
                test_name = Some(line.replace("// Name: ", "").trim().to_string());
            }
        }

        if test_name.is_some() {
            tests.push(Test {
                name: test_name.unwrap(),
                content: test_content,
                file: file_path.clone()
            });
        }
    }

    let results: Vec<_> = tests
        .par_iter()
        .map(|test: &Test| {
            let value = run_test(test.name.clone(), test.content.clone());
            (test.file.clone(), value)
        })
        .collect();

    let mut grouped: HashMap<String, Vec<TestOutput>> = HashMap::new();

    for (key, result) in results {
        grouped.entry(key).or_default().push(result);
    }

    for (file, tasks) in grouped {
        println!("\n{style_underline}{style_bold}Running tests in {file}{style_reset}");
        for task in tasks {
            println!("\n{style_bold}> {}{style_reset}", task.name);

            match task.result {
                Ok(duration) => {
                    println!(
                        "  {color_blue}Execution time{color_reset}: {color_yellow}{:.2?}{color_reset}",
                        duration
                    );
                    println!("  {color_bright_green}Test passed!{color_reset}")
                },
                Err(e) => println!("  {color_bright_red}Test failed: {}{color_reset}", e),
            }
        }
    };

    panic::set_hook(default_hook);
}

fn run_test(name: String, content: String) -> TestOutput {
    let result = catch_unwind(|| {
        // TODO: replace all unwrap by proper handling
        let tokens = lexer::lex(content.clone()).unwrap();

        let ast = parser::parse(tokens).unwrap();

        let ir = translator::translate(ast);

        let now = Instant::now();

        let program = Rc::new(RefCell::new(ProgramContext::new()));

        let context = ModuleContext::new(program);

        let context_ref = Rc::new(RefCell::new(context));

        context_ref.borrow_mut().environment.init(context_ref.clone());

        let result = interpreter::interpret(ir, context_ref);

        let elapsed = now.elapsed();

        TestOutput {
            name: name.clone(),
            result: match result {
                Ok(_) => Ok(elapsed),
                Err(e) => Err(e.format(Some(ErrorSource {
                    contents: Rc::new(RefCell::new(content)),
                    path: name.clone()
                }))),
            }
        }
    });

    match result {
        Ok(output) => output,
        Err(err) => {
            let err = if let Some(msg) = err.downcast_ref::<&str>() {
                Err(msg.to_string())
            } else if let Some(msg) = err.downcast_ref::<String>() {
                Err(msg.to_string())
            } else {
                Err("Caught unknown panic type".to_string())
            };
            
            TestOutput {
                name,
                result: err
            }
        }
    }
}