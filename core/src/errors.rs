use std::{cell::RefCell, env, fmt::Debug, path::{MAIN_SEPARATOR_STR, PathBuf}, rc::Rc, sync::{Arc}};
use inline_colorization::*;
use parking_lot::Mutex;

pub trait LanguageErrorTrait: Debug + Send + Sync {}

#[derive(Debug, Clone)]
pub struct ErrorSource {
    pub contents: Arc<Mutex<String>>,
    pub path: String
}

#[derive(Debug)]
pub struct LanguageError {
    pub kind: Box<dyn LanguageErrorTrait>,
    pub source_bindings: Option<(usize, usize)>,
    pub source: Option<ErrorSource>
}

struct LineInfo {
    pub index: usize,
    pub start: usize,
    pub end: usize,
    pub content: String
}


pub fn provide_source<T>(result: &mut LanguageResult<T>, source: ErrorSource) {
    if let Err(err) = result {
        if err.source.is_none() {
            err.source = Some(source)
        }
    }
}

impl LanguageError {
    pub fn from(kind: impl LanguageErrorTrait + 'static) -> Self {
        Self {
            kind: Box::new(kind),
            source_bindings: None,
            source: None
        }
    }

    pub fn with_source(kind: impl LanguageErrorTrait + 'static, start: usize, end: usize) -> Self {
        Self {
            kind: Box::new(kind),
            source_bindings: Some((start, end)),
            source: None
        }
    }

    pub fn format(&self, source: Option<ErrorSource>) -> String {
        let source = if let Some(source) = source {
            source
        } else if let Some(source) = &self.source {
            source.clone()
        } else {
            panic!()
        };

        let contents = source.contents.lock();

        fn get_line_bounds(cursor: usize, contents: String) -> LineInfo {
            let mut line_start = 0;
            let mut index = 1;

            for (i, c) in contents.char_indices() {
                if c == '\n' {
                    let line_end = i;

                    if (line_start..=line_end).contains(&cursor) {
                        return LineInfo {
                            index,
                            start: line_start,
                            end: line_end,
                            content: contents[line_start..line_end].to_string(),
                        };
                    }

                    line_start = i + 1;
                    index += 1;
                }
            }

            // last line
            if (line_start..=contents.len()).contains(&cursor) {
                return LineInfo {
                    index,
                    start: line_start,
                    end: contents.len(),
                    content: contents[line_start..].to_string(),
                };
            }

            unreachable!()
        }

        let formatted_error = Rc::new(RefCell::new(String::new()));

        formatted_error.borrow_mut().push_str(format!("{style_bold}{color_red}error: {color_reset}{style_reset}{:?}\n", self.kind).as_str());

        if let Some((start, end)) = self.source_bindings {
            let add_line = |line: String, string: String| {
                formatted_error.borrow_mut().push_str(format!("{style_bold}{color_bright_white}{:>2} |{color_reset}{style_reset} ", line).as_str());
                formatted_error.borrow_mut().push_str(string.as_str());
            };

            let first_line = get_line_bounds(start, contents.to_string());

            let last_line = get_line_bounds(end, contents.to_string());

            let path = source.path.replace(&(PathBuf::from(env::current_dir().unwrap()).canonicalize().unwrap().to_str().unwrap().to_string() + MAIN_SEPARATOR_STR), "");

            formatted_error.borrow_mut().push_str(format!("  {style_bold}{color_bright_white}--> {}:{}:{}{color_reset}{style_reset}\n", path, first_line.index, start - first_line.start + 1).as_str());

            let indicator_pos =
                contents[first_line.start..start].chars().count();
            let indicator_len =
                contents[start..end].chars().count();
            if first_line.start == last_line.start {
                add_line(first_line.index.to_string(), format!("{}\n", first_line.content));
                add_line("".into(), format!("{color_red}{style_bold}{}{}{color_reset}{style_reset}", " ".repeat(indicator_pos), "^".repeat(indicator_len)));  
            } else {
                add_line(first_line.index.to_string(), format!("{}\n", first_line.content));
                add_line("".into(), format!("{color_red}{style_bold}{}^{color_reset}{style_reset}\n", " ".repeat(indicator_pos)));
                formatted_error.borrow_mut().push_str(format!("{style_bold}{color_bright_white}...{color_reset}{style_reset}\n").as_str());
                let indicator_pos =
                    contents[last_line.start..last_line.end].chars().count();
                add_line(last_line.index.to_string(), format!("{}\n", last_line.content));
                add_line("".into(), format!("{color_red}{style_bold}{}^{color_reset}{style_reset}\n", " ".repeat(indicator_pos - 1)));
            }
        }

        // println!("{}", &contents[self.start..self.end]);

        // let indicator_pos = self.start - start;
        // println!("{color_yellow}{}^{color_reset}", " ".repeat(indicator_pos - 1));

        formatted_error.take()
    }
}

pub type LanguageResult<T> = Result<T, LanguageError>;

#[derive(Debug)]
pub enum LexerError {
    UnterminatedString
}

impl LanguageErrorTrait for LexerError {}

