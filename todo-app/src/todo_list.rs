use crate::Todo;
use std::io::Write;

// 파일 포맷
// - 엔터로 구분되어있는 형식
// 예) (content)\n(deadline)\n...
pub(crate) struct TodoList {
    todos: Vec<Todo>,
}

impl TodoList {
    pub(crate) fn init() -> Self {
        match std::fs::read("todos") {
            Ok(todos) => parse_todos(todos),
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => Self { todos: Vec::new() },
                _ => {
                    panic!("Failed to read todos file: {}", err);
                }
            },
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Todo> {
        self.todos.iter()
    }

    pub(crate) fn push(&mut self, todo: Todo) {
        self.todos.push(todo);
        self.save();
    }

    pub(crate) fn remove(&mut self, index: usize) {
        self.todos.remove(index);
        self.save();
    }

    pub(crate) fn mutate(&mut self, index: usize, mutate: impl FnOnce(&mut Todo)) {
        let Some(todo) = self.todos.get_mut(index) else {
            return;
        };
        mutate(todo);
        self.save();
    }

    pub(crate) fn len(&self) -> usize {
        self.todos.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.todos.is_empty()
    }

    fn save(&self) {
        let mut todos_file = std::fs::File::create("todos").unwrap();
        for todo in &self.todos {
            todos_file
                .write_all(format!("{}\n", todo.content).as_bytes())
                .unwrap();
            todos_file
                .write_all(format!("{}\n", todo.deadline.as_deref().unwrap_or("N/A")).as_bytes())
                .unwrap();
        }
    }
}

fn parse_todos(todos: Vec<u8>) -> TodoList {
    let todos_string = String::from_utf8(todos).unwrap();
    let mut todos = Vec::new();

    let mut lines = todos_string.lines();

    while let Some(content) = lines.next() {
        let deadline = lines.next().unwrap();
        todos.push(Todo {
            content: content.to_string(),
            deadline: if deadline == "N/A" {
                None
            } else {
                Some(deadline.to_string())
            },
        });
    }

    TodoList { todos }
}
