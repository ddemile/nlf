use inline_colorization::*;
use std::{
    fs,
    panic::{self, catch_unwind},
    path::{Path},
    time::Instant,
};

use crate::{interpreter, lexer, parser, translator};

pub fn run_tests(base_dir: &Path) {
    let paths = fs::read_dir(base_dir).unwrap();

    // Save the original hook
    let default_hook = panic::take_hook();

    // Replace it with a no-op hook to suppress printing
    panic::set_hook(Box::new(|_| {}));

    for entry in paths {
        let path = entry.unwrap().path();
        let contents = fs::read_to_string(&path).unwrap();
        let lines = contents.lines().collect::<Vec<&str>>();

        let mut test_name: Option<String> = None;
        let mut test_content = String::new();

        println!(
            "\n{style_underline}{style_bold}Running tests in {path}{style_reset}",
            path = format!(
                "{}/{}",
                base_dir.display(),
                path.file_name().unwrap().to_str().unwrap()
            )
        );

        fn run_test(name: String, content: String) -> Result<(), String> {
            let result = catch_unwind(|| {
                println!("\n{style_bold}> {name}{style_reset}");

                let tokens = lexer::lex(content);

                let ast = parser::parse(tokens);

                let ir = translator::translate(ast);

                let now = Instant::now();
                let result = interpreter::interpret(ir);

                let elapsed = now.elapsed();

                println!(
                    "  {color_blue}Execution time{color_reset}: {color_yellow}{:.2?}{color_reset}",
                    elapsed
                );

                match result {
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!("{}", e)),
                }
            });

            match result {
                Ok(Err(message)) => Err(message),
                Ok(_) => Ok(()),
                Err(err) => {
                    if let Some(msg) = err.downcast_ref::<&str>() {
                        Err(msg.to_string())
                    } else if let Some(msg) = err.downcast_ref::<String>() {
                        Err(msg.to_string())
                    } else {
                        Err("Caught unknown panic type".to_string())
                    }
                }
            }
        }

        for line in lines {
            if test_name.is_some() {
                test_content.push_str(format!("{}\n", line).as_str());
            }

            if line.starts_with("// Name: ") {
                if test_name.is_some() {
                    match run_test(test_name.unwrap(), test_content) {
                        Ok(_) => println!("  {color_bright_green}Test passed!{color_reset}"),
                        Err(e) => println!("  {color_bright_red}Test failed: {}{color_reset}", e),
                    }
                }
                test_content = String::new();
                test_name = Some(line.replace("// Name: ", "").trim().to_string());
            }
        }

        if test_name.is_some() {
            match run_test(test_name.unwrap(), test_content) {
                Ok(_) => println!("  {color_bright_green}Test passed!{color_reset}"),
                Err(e) => println!("  {color_bright_red}Test failed: {}{color_reset}", e),
            }
        }
    }

    panic::set_hook(default_hook);
}
