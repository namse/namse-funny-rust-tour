use crate::{todo_list::TodoList, todo_selector::selector, Todo};
use std::io::Write;

/*
아래는 선택기의 모습입니다.
```
┌────────┬──────────────────────────┬────────────┐
│ Select │              TODO        │  DeadLine  │
├────────┼──────────────────────────┼────────────│
│    *   │ Eat Food                 │ 2023-10-05 │ // <- *이 선택된 것.
├────────┼──────────────────────────┼────────────│
│        │ Sleep                    │    N/A     │
└────────┴──────────────────────────┴────────────┘
```

Q. 어떤 상태가 존재합니까?
- 내가 지금 무엇에 선택 커서를 올렸는지

*/

pub(crate) fn edit_todo(todo_list: &mut TodoList) {
    let selected_todo_index = match selector(todo_list) {
        Ok(index) => index,
        Err(err) => match err {
            crate::todo_selector::SelectorError::EmptyTodoList => {
                println!("There is no TODO to edit!");
                return;
            }
        },
    };
    todo_list.mutate(selected_todo_index, |todo| {
        edit_with_prompt(todo);
    });

    println!("Success!");
}

fn edit_with_prompt(todo: &mut Todo) {
    println!("Edit a TODO");
    print!("- TODO [{}]: ", todo.content);
    std::io::stdout().flush().unwrap();

    let mut content = String::new();
    std::io::stdin().read_line(&mut content).unwrap();
    if content != "\n" {
        todo.content = content.trim().to_string();
    }

    println!("(enter 'n' for no deadline or 'yyyy-mm-dd')");
    print!(
        "- DeadLine [{}]: ",
        todo.deadline.as_deref().unwrap_or("N/A")
    );
    std::io::stdout().flush().unwrap();

    let mut deadline = String::new();
    std::io::stdin().read_line(&mut deadline).unwrap();
    if deadline != "\n" {
        if deadline.trim() == "n" {
            todo.deadline = None;
        } else {
            todo.deadline = Some(deadline.trim().to_string());
        }
    }
}
