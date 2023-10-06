use crate::{todo_list::TodoList, todo_selector::selector};

pub(crate) fn delete_todo(todo_list: &mut TodoList) {
    let selected_todo_index = match selector(todo_list) {
        Ok(index) => index,
        Err(err) => match err {
            crate::todo_selector::SelectorError::EmptyTodoList => {
                println!("There is no TODO to delete!");
                return;
            }
        },
    };
    todo_list.remove(selected_todo_index);

    println!("Success!");
}
