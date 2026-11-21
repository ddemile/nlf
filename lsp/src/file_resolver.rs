use std::{collections::HashMap, env, fs, hash::Hash, path::{Path, PathBuf}, rc::Rc, str::FromStr};

use nlf_core::{compiler::resolvers::{FileSystemModuleResolver, ModuleResolver}, lexer, loader::{self, ModuleKind, ModuleSource}, manifest::search_manifest, parser::{self, Statement, StatementKind}, type_experiments::{AbstractType, ProgramContext, TypeAllocator, TypeCollector, type_program}};
use parking_lot::RwLock;
use serde::de;
use tower_lsp::lsp_types::{TextDocumentItem, Url};

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileId(u32);

pub struct DocumentStore {
    pub map: HashMap<Url, FileId>,
    file_store: RwLock<FileStore>
}

impl DocumentStore {
    pub fn new() -> Self {
       Self { map: HashMap::new(), file_store: RwLock::new(FileStore { map: HashMap::new() }) } 
    }

    pub fn add_document(&mut self, document: &TextDocumentItem) {
        let path = document.uri.to_file_path().unwrap();

        let file_id = self.file_store.write().get_or_create_file(path, document.text.clone()).unwrap();

        self.map.insert(document.uri.clone(), file_id);
    }

    pub fn try_update_document(&mut self, document: &TextDocumentItem) {
        let file_store = self.file_store.read();

        let file_id = file_store.get_file_id(document.uri.to_file_path().unwrap()).unwrap();

        let file = file_store.get_file(file_id);

        if file.contents == document.text {
            return
        }
    }

    pub fn debug(&self) -> Vec<String> {
        let mut vec = vec![];

        for (id, file) in self.file_store.read().map.iter() {
            let mut dependencies = vec![];

            for dep in file.dependencies.iter() {
                dependencies.push(self.file_store.read().map.get(&dep).unwrap().path.file_name().unwrap().to_str().unwrap().to_string());
            }

            vec.push(format!("{} : {}", file.path.file_name().unwrap().to_str().unwrap().to_string(), dependencies.join(", ")));
        }

        vec
    }
}

pub struct File {
    id: FileId,
    path: PathBuf,
    contents: String,
    dependencies: Vec<FileId>,
    dirty: bool,
    type_collector: TypeCollector
}

impl File {
    // pub fn get_children(id: FileId, store: &mut FileStore) -> Vec<FileId> {
    //     let dependencies = match store.map.get_mut(&id) {
    //         Some(file) => {
    //             file.dirty = true;
    //             file.dependencies.clone()
    //         }
    //         None => return,
    //     };

    //     for dependency in dependencies {
    //         Self::get_children(dependency, store);
    //     }
    // }
}

struct FileData {
    type_collector: TypeCollector,
    dependencies: Vec<FileId>
}


impl FileStore {
    fn generate_file_date<P: AsRef<Path>>(&mut self, path: P, contents: String) -> Result<FileData, ()> {
        let Ok(tokens) = lexer::lex(contents.clone()) else {
            return Err(())
        };

        let Ok(ast) = parser::parse(tokens) else {
            return Err(())
        };

        let mut dependencies = vec![];

        let resolver: Rc<dyn ModuleResolver> = Rc::new(FileSystemModuleResolver {
            process_path: env::current_dir().unwrap()
        });

        for statement in ast.body.iter() {
            if let Statement { kind: StatementKind::Import { source, .. }, .. } = statement {
                let module_source = resolver.resolve_source(&source.value, Some(path.as_ref().parent().unwrap().to_path_buf())).unwrap();

                let contents = resolver.read(&module_source).unwrap();

                let file_id = self.get_or_create_file(module_source.path, contents).unwrap();

                dependencies.push(file_id);
            }
        }

        let mut allocator: TypeAllocator<AbstractType> = TypeAllocator::new();

        let mut context = ProgramContext {
            namespace: allocator.create_namespace()
        };

        let (_, type_collector) = type_program(ast, &mut context).unwrap();

        let file_data = FileData {
            type_collector,
            dependencies,
        };

        Ok(file_data)
    }

    pub fn create_file<P: AsRef<Path>>(&mut self, path: P, contents: String) -> Result<FileId, ()> {
        let file_data = self.generate_file_date(&path, contents.clone())?;

        let file_id = FileId(self.map.len() as u32);

        let file = File {
            id: file_id,
            path: path.as_ref().into(),
            contents,
            type_collector: file_data.type_collector,
            dependencies: file_data.dependencies,
            dirty: false
        };

        self.map.insert(file.id, file);

        Ok(file_id)
    }

    pub fn get_or_create_file<P: AsRef<Path>>(&mut self, path: P, contents: String) -> Result<FileId, ()> {
        if let Some(file_id) = self.get_file_id(&path) {
            Ok(file_id)
        } else {
            self.create_file(path, contents)
        }
    }

    pub fn get_file_id<P: AsRef<Path>>(&self, path: P) -> Option<FileId> {
        for file in self.map.values() {
            if file.path == path.as_ref() {
                return Some(file.id)
            }
        }

        None
    }

    pub fn get_file(&self, id: FileId) -> &File {
        &self.map[&id]
    }

    pub fn update_file<P: AsRef<Path>>(&mut self, path: P, contents: String) -> Result<(), ()> {
        let file_data = self.generate_file_date(&path, contents)?;

        let file_id = self.get_file_id(path).unwrap();

        // File::get_children(file_id, self);

        let file = self.map.get_mut(&file_id).unwrap();
        
        file.type_collector = file_data.type_collector;
        file.dependencies = file_data.dependencies;
        
        Ok(())
    }

    pub fn get_tree(&mut self, id: FileId) -> Vec<FileId> {
        let dependencies = self.get_file(id).dependencies.clone();

        let mut children = vec![];

        for child in dependencies {
            children.push(child);
            let mut recursive_children = self.get_tree(child);
            children.append(&mut recursive_children);
        }

        children
    }

    // pub fn clean(&mut self) {
    //     let file_data = self.generate_file_date(&path, contents)?;

    //     let file_id = self.get_file_id(path).unwrap();

    //     // File::get_children(file_id, self);

    //     let file = self.map.get_mut(&file_id).unwrap();
        
    //     file.type_collector = file_data.type_collector;
    //     file.dependencies = file_data.dependencies;
    // }
}

pub struct FileStore {
    pub map: HashMap<FileId, File>
}