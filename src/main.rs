use opener::open;
use rustyline::DefaultEditor;
use std::{
    collections::HashMap,
    fs::{self, read_dir},
    io::{Read, Write},
    path::Path,
    process::Command,
};
type FileObj = std::fs::File;

fn main() {
    println!("Objective Shell");
    let mut rl = DefaultEditor::new().unwrap();
    let mut sh = Shell {
        memory: HashMap::from([(
            "Current-Folder".to_string(),
            Type::Folder(Folder {
                path: format!("{}", std::env::current_dir().unwrap().display()),
            }),
        )]),
    };

    loop {
        let order = rl.readline(">>> ").unwrap().trim().to_string();
        if order.is_empty() {
            continue;
        }
        if let Some(result) = sh.run(order) {
            println!("{}", result.display());
        }
    }
}

#[derive(Debug, Clone)]
struct Shell {
    memory: HashMap<String, Type>,
}
impl Shell {
    fn set_current_folder(&self) -> Option<()> {
        std::env::set_current_dir(
            if let Type::Folder(Folder { path }) = self.memory.get("Current-Folder")? {
                path
            } else {
                return None;
            },
        )
        .unwrap();
        Some(())
    }

    fn run(&mut self, source: String) -> Option<Type> {
        let source = tokenize_program(source);
        let mut result: Option<Type> = None;
        for lines in source {
            self.set_current_folder();
            if lines.len() == 2 {
                result = self.eval(lines[1].to_string());
                self.memory
                    .insert(lines[0].trim().to_string(), result.clone()?);
            } else {
                result = self.eval(lines[0].to_string());
            }
        }
        result
    }

    fn eval(&mut self, program: String) -> Option<Type> {
        let line = tokenize_expr(program);
        let obj = Type::parse(line[0].clone(), self.memory.clone())?;
        if line.len() > 1 {
            let method = line[1].clone();
            let args: Vec<String> = line.get(2..).unwrap_or([].as_slice()).to_vec();
            let args: Vec<Type> = {
                let mut new = vec![];
                for i in args {
                    new.push(Type::parse(i, self.memory.clone())?);
                }
                new
            };

            match obj {
                Type::File(mut file) => match method.as_str() {
                    "Read-String" => Some(Type::String(file.read())),
                    "Open" => {
                        file.open();
                        None
                    }
                    "Write-String" => {
                        file.write(args[0].get_string()?);
                        None
                    }
                    "Rename" => {
                        file.rename(args[0].get_string()?);
                        None
                    }
                    "Delete" => {
                        file.delete();
                        None
                    }
                    _ => None,
                },
                Type::Folder(mut folder) => match method.as_str() {
                    "Item-List" => Some(Type::Array(folder.item_list())),
                    "Rename" => {
                        folder.rename(args[0].get_string()?);
                        None
                    }
                    "Delete" => {
                        folder.delete();
                        None
                    }
                    _ => None,
                },
                Type::App(mut app) => match method.as_str() {
                    "Start" => {
                        if let Some(arg) = args.get(0..) {
                            app.start_with_arg(
                                arg.iter()
                                    .map(|x| x.get_string().unwrap_or_default())
                                    .collect(),
                            );
                        } else {
                            app.start();
                        }
                        None
                    }
                    _ => None,
                },
                Type::Number(i) => match method.as_str() {
                    "+" => Some(Type::Number(i + args[0].get_number()?)),
                    "-" => Some(Type::Number(i - args[0].get_number()?)),
                    "*" => Some(Type::Number(i * args[0].get_number()?)),
                    "/" => Some(Type::Number(i / args[0].get_number()?)),
                    "%" => Some(Type::Number(i % args[0].get_number()?)),
                    "^" => Some(Type::Number(i.powf(args[0].get_number()?))),
                    _ => None,
                },
                Type::String(s) => match method.as_str() {
                    "+" => Some(Type::String(s + &args[0].get_string()?)),
                    "PrintLn" => {
                        println!("{s}");
                        None
                    }
                    _ => None,
                },
                Type::Array(array) => match method.as_str() {
                    "Index" => Some(array.get(args[0].get_number()? as usize)?.clone()),
                    "Length" => Some(Type::Number(array.len() as f64)),
                    _ => None,
                },
            }
        } else {
            Some(obj)
        }
    }
}

fn tokenize_expr(input: String) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_parentheses: usize = 0;
    let mut in_quote = false;

    for c in input.chars() {
        match c {
            '(' if !in_quote => {
                in_parentheses += 1;
                current_token.push(c);
            }
            ')' if !in_quote => {
                if in_parentheses != 0 {
                    current_token.push(c);
                    in_parentheses -= 1;
                    if in_parentheses == 0 {
                        tokens.push(current_token.clone());
                        current_token.clear();
                    }
                }
            }
            '[' if !in_quote => {
                in_parentheses += 1;
                current_token.push(c);
            }
            ']' if !in_quote => {
                if in_parentheses != 0 {
                    current_token.push(c);
                    in_parentheses -= 1;
                    if in_parentheses == 0 {
                        tokens.push(current_token.clone());
                        current_token.clear();
                    }
                }
            }
            '{' if !in_quote => {
                in_parentheses += 1;
                current_token.push(c);
            }
            '}' if !in_quote => {
                if in_parentheses != 0 {
                    current_token.push(c);
                    in_parentheses -= 1;
                    if in_parentheses == 0 {
                        tokens.push(current_token.clone());
                        current_token.clear();
                    }
                }
            }
            '"' => {
                if in_parentheses == 0 {
                    if in_quote {
                        current_token.push(c);
                        in_quote = false;
                        tokens.push(current_token.clone());
                        current_token.clear();
                    } else {
                        in_quote = true;
                        current_token.push(c);
                    }
                } else {
                    current_token.push(c);
                }
            }
            ' ' | '\n' | '\t' | '\r' | '　' => {
                if in_parentheses != 0 || in_quote {
                    current_token.push(c);
                } else if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
            }
            _ => {
                current_token.push(c);
            }
        }
    }

    if !(in_parentheses != 0 || in_quote || current_token.is_empty()) {
        tokens.push(current_token);
    }
    tokens
}

fn tokenize_program(input: String) -> Vec<Vec<String>> {
    let mut tokens: Vec<Vec<String>> = Vec::new();
    let mut current_token = String::new();
    let mut after_equal = String::new();
    let mut is_equal = false;
    let mut in_parentheses: usize = 0;
    let mut in_quote = false;

    for c in input.chars() {
        match c {
            '{' if !in_quote => {
                if is_equal {
                    after_equal.push(c);
                } else {
                    current_token.push(c);
                }
                in_parentheses += 1;
            }
            '}' if !in_quote => {
                if is_equal {
                    after_equal.push(c);
                } else {
                    current_token.push(c);
                }
                in_parentheses -= 1;
            }
            ';' if !in_quote => {
                if in_parentheses != 0 {
                    if is_equal {
                        after_equal.push(c);
                    } else {
                        current_token.push(c);
                    }
                } else if !current_token.is_empty() {
                    if is_equal {
                        is_equal = false;
                        tokens.push(vec![current_token.clone(), after_equal.clone()]);
                        current_token.clear();
                        after_equal.clear();
                    } else {
                        tokens.push(vec![current_token.clone()]);
                        current_token.clear();
                    }
                }
            }
            '=' if !in_quote => {
                if in_parentheses != 0 {
                    if is_equal {
                        after_equal.push(c);
                    } else {
                        current_token.push(c);
                    }
                } else {
                    is_equal = true;
                }
            }
            '"' => {
                in_quote = !in_quote;
                if is_equal {
                    after_equal.push(c);
                } else {
                    current_token.push(c);
                }
            }
            _ => {
                if is_equal {
                    after_equal.push(c);
                } else {
                    current_token.push(c);
                }
            }
        }
    }

    if in_parentheses == 0 && !current_token.is_empty() {
        if is_equal {
            tokens.push(vec![current_token.clone(), after_equal]);
            current_token.clear();
        } else {
            tokens.push(vec![current_token.clone()]);
            current_token.clear();
        }
    }
    tokens
}

#[derive(Debug, Clone)]
enum Type {
    File(File),
    Folder(Folder),
    App(App),
    Number(f64),
    String(String),
    Array(Vec<Type>),
}

impl Type {
    fn parse(source: String, memory: HashMap<String, Type>) -> Option<Type> {
        let mut source = source.trim().to_string();
        if let Some(value) = memory.get(&source) {
            Some(value.clone())
        } else if let Ok(i) = source.parse::<f64>() {
            Some(Type::Number(i))
        } else if source.starts_with('"') && source.ends_with('"') {
            Some(Type::String({
                source.remove(source.find('"').unwrap_or_default());
                source.remove(source.rfind('"').unwrap_or_default());
                source.to_string()
            }))
        } else if source.starts_with('(') && source.ends_with(')') {
            source.remove(source.find('(').unwrap_or_default());
            source.remove(source.rfind(')').unwrap_or_default());
            Shell { memory }.eval(source.to_string())
        } else if source.starts_with('{') && source.ends_with('}') {
            source.remove(source.find('{').unwrap_or_default());
            source.remove(source.rfind('}').unwrap_or_default());
            Shell { memory }.run(source.to_string())
        } else if source.starts_with('[') && source.ends_with(']') {
            Some(Type::Array({
                source.remove(source.find('[').unwrap_or_default());
                source.remove(source.rfind(']').unwrap_or_default());
                tokenize_expr(source.to_string())
                    .iter()
                    .map(|x| Type::parse(x.to_string(), memory.clone()).unwrap())
                    .collect()
            }))
        } else if source.starts_with("File(") && source.ends_with(')') {
            source = source.replacen("File(", "", 1);
            source.remove(source.rfind(')').unwrap_or_default());
            Some(Type::File(File::new(
                Shell { memory }.eval(source.to_string())?.get_string()?,
            )?))
        } else if source.starts_with("Folder(") && source.ends_with(')') {
            source = source.replacen("Folder(", "", 1);
            source.remove(source.rfind(')').unwrap_or_default());
            Some(Type::Folder(Folder::new(
                Shell { memory }.eval(source.to_string())?.get_string()?,
            )))
        } else if source.starts_with("App(") && source.ends_with(')') {
            source = source.replacen("App(", "", 1);
            source.remove(source.rfind(')').unwrap_or_default());
            Some(Type::App(App::new(
                Shell { memory }.eval(source.to_string())?.get_string()?,
            )))
        } else {
            Some(Type::String(source))
        }
    }

    fn get_string(&self) -> Option<String> {
        if let Type::String(s) = self {
            Some(s.to_string())
        } else if let Type::Number(n) = self {
            Some(n.to_string())
        } else if let Type::File(File { path }) = self {
            Some(path.to_string())
        } else if let Type::Folder(Folder { path }) = self {
            Some(path.to_string())
        } else {
            None
        }
    }

    fn get_number(&self) -> Option<f64> {
        if let Type::Number(i) = self {
            Some(*i)
        } else if let Ok(i) = self.get_string()?.trim().parse::<f64>() {
            Some(i)
        } else {
            None
        }
    }

    fn display(&self) -> String {
        match self {
            Type::App(App { name }) => format!("App( {name} )"),
            Type::File(File { path }) => format!("File( {path} )"),
            Type::Folder(Folder { path }) => format!("File( {path} )"),
            Type::Number(n) => n.to_string(),
            Type::String(s) => format!("\"{s}\""),
            Type::Array(a) => format!(
                "[{}]",
                a.iter()
                    .map(|x| x.display())
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct File {
    path: String,
}

fn open_file(path: String) -> FileObj {
    FileObj::options()
        .create(true)
        .write(true)
        .read(true)
        .open(Path::new(&path))
        .unwrap()
}

impl File {
    fn new(path: String) -> Option<File> {
        if !Path::new(&path).exists() {
            FileObj::create(path.clone()).unwrap();
        }
        let path = if Path::new(&path).is_relative() {
            fs::canonicalize(&path)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        } else {
            path
        };
        Some(File { path })
    }

    fn write(&mut self, value: String) {
        open_file(self.path.clone())
            .write_all(value.as_bytes())
            .unwrap();
    }

    fn read(&mut self) -> String {
        let buf = &mut String::new();
        open_file(self.path.clone())
            .read_to_string(buf)
            .unwrap_or_default();
        buf.to_owned()
    }

    fn delete(&mut self) {
        fs::remove_file(self.path.clone()).unwrap_or_default();
    }

    fn rename(&mut self, name: String) {
        fs::rename(self.path.clone(), name).unwrap_or_default();
    }

    fn open(&mut self) {
        open(self.path.clone()).unwrap_or_default();
    }
}

#[derive(Debug, Clone)]
struct Folder {
    path: String,
}

impl Folder {
    fn new(path: String) -> Folder {
        if !Path::new(&path).exists() {
            fs::create_dir(path.clone()).unwrap();
        }
        let path = if Path::new(&path).is_relative() {
            fs::canonicalize(&path)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        } else {
            path
        };
        Folder { path }
    }

    fn item_list(&mut self) -> Vec<Type> {
        let mut list = vec![];
        for entry in read_dir(self.path.clone()).unwrap() {
            let path = &format!("{}", entry.unwrap().path().display());
            if Path::new(path).is_file() {
                list.push(Type::File(File {
                    path: path.to_string(),
                }))
            } else if Path::new(path).is_dir() {
                list.push(Type::Folder(Folder {
                    path: path.to_string(),
                }))
            }
        }
        list
    }

    fn delete(&mut self) {
        fs::remove_dir_all(self.path.clone()).unwrap();
    }

    fn rename(&mut self, name: String) {
        fs::rename(self.path.clone(), name).unwrap();
    }
}

#[derive(Debug, Clone)]
struct App {
    name: String,
}

impl App {
    fn new(name: String) -> App {
        App { name }
    }

    fn start(&mut self) {
        Command::new(self.name.clone())
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }

    fn start_with_arg(&mut self, arg: Vec<String>) {
        Command::new(self.name.clone())
            .args(arg)
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }
}
