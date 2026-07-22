use std::{collections::HashMap, env, fs, path::PathBuf};

use crate::{errors::{LanguageError, LanguageResult}, loader::LoaderError, new_loader::{ModuleKind, ModuleSource}, stdlib::CoreModules};

pub trait ModuleResolver {
    fn resolve_path(&self, path: PathBuf, current_path: &PathBuf) -> LanguageResult<PathBuf>;
    fn resolve_source(&self, source: &str, current_folder: Option<PathBuf>) -> LanguageResult<ModuleSource>;

    fn read(&self, source: &ModuleSource) -> LanguageResult<String>;
}

pub struct FileSystemModuleResolver {
    pub process_path: PathBuf
}

impl ModuleResolver for FileSystemModuleResolver {
    fn resolve_path(&self, path: PathBuf, current_path: &PathBuf) -> LanguageResult<PathBuf> {
        let base_path = {
            if path.starts_with("./") || path.starts_with("../") {
                current_path
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

    fn resolve_source(&self, source: &str, current_folder: Option<PathBuf>) -> LanguageResult<ModuleSource> {
        let current_dir = env::current_dir().unwrap();

        let current_path = if current_folder.is_some() { current_folder.unwrap() } else { current_dir };

        let resolved = match self.resolve_path(PathBuf::from(source), &current_path).map(|buf| buf.to_str().unwrap().to_string()) {
            Ok(resolved) => resolved,
            Err(_) => source.to_string()
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

    fn read(&self, source: &ModuleSource) -> LanguageResult<String> {
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
}

#[derive(Clone)]
pub struct NonImplementedModuleResolver;

impl ModuleResolver for NonImplementedModuleResolver {
    fn resolve_path(&self, _path: PathBuf, _current_path: &PathBuf) -> LanguageResult<PathBuf> {
        panic!("You shouldn't try to resolve a path using the NonImplementedPathResolver")
    }
    
    fn resolve_source(&self, _source: &str, _current_folder: Option<PathBuf>) -> LanguageResult<ModuleSource> {
        panic!("You shouldn't try to resolve a module source using the NonImplementedPathResolver")
    }

    fn read(&self, _source: &ModuleSource) -> LanguageResult<String> {
        panic!("You shouldn't try to read a module using the NonImplementedPathResolver")
    }
}

/// A simple path resolver that just takes a list of module and their contents
pub struct StaticModuleResolver {
    pub modules: HashMap<String, String>
}

impl ModuleResolver for StaticModuleResolver {
    fn resolve_path(&self, path: PathBuf, _current_path: &PathBuf) -> LanguageResult<PathBuf> {
        Ok(path)
    }

    fn resolve_source(&self, source: &str, _current_folder: Option<PathBuf>) -> LanguageResult<ModuleSource> {
        Ok(ModuleSource {
            kind: ModuleKind::Standard,
            path: source.to_string()
        })
    }

    fn read(&self, source: &ModuleSource) -> LanguageResult<String> {
        let contents = self.modules.get(&source.path).ok_or_else(|| {
            LanguageError::from(LoaderError::TODO)
        })?;

        Ok(contents.to_string())
    }
}