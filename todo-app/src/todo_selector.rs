use crate::todo_list::TodoList;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

/// returns selected todo index
pub(crate) fn selector(todo_list: &TodoList) -> Result<usize, SelectorError> {
    if todo_list.is_empty() {
        return Err(SelectorError::EmptyTodoList);
    }

    let mut select_cursor = 0;

    loop {
        draw_selector(todo_list, select_cursor);

        let input: Input = get_input();
        match input {
            Input::Up => select_up(&mut select_cursor),
            Input::Down => select_down(&mut select_cursor, todo_list),
            Input::Enter => {
                return Ok(select_cursor);
            }
        }

        crossterm::execute!(
            std::io::stdout(),
            crossterm::cursor::MoveToPreviousLine(drawing_line_height(todo_list) as u16),
        )
        .unwrap();
    }
}

fn drawing_line_height(todo_list: &TodoList) -> usize {
    todo_list.len() * 2 + 3
}

pub(crate) enum SelectorError {
    EmptyTodoList,
}

fn draw_selector(todo_list: &TodoList, select_cursor: usize) {
    println!("┌────────┬──────────────────────────┬────────────┐");
    println!("│ Select │              TODO        │  DeadLine  │");
    for (i, todo) in todo_list.iter().enumerate() {
        println!("├────────┼──────────────────────────┼────────────│");
        print!("│    {}   ", if i == select_cursor { "*" } else { " " });
        print!("│ {: <24} ", todo.content);
        let deadline = match &todo.deadline {
            Some(deadline) => deadline,
            None => "N/A",
        };
        println!("│ {: <10} │", deadline);
    }
    println!("└────────┴──────────────────────────┴────────────┘");
}

enum Input {
    Up,
    Down,
    Enter,
}

fn select_down(select_cursor: &mut usize, todo_list: &TodoList) {
    if *select_cursor == todo_list.len() - 1 {
        return;
    }
    *select_cursor += 1;
}

fn select_up(select_cursor: &mut usize) {
    if *select_cursor == 0 {
        return;
    }

    *select_cursor -= 1;
}

fn get_input() -> Input {
    enable_raw_mode().unwrap();
    loop {
        let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() else {
            continue;
        };

        if key.kind != crossterm::event::KeyEventKind::Press {
            continue;
        }

        let input = match key.code {
            crossterm::event::KeyCode::Up => Input::Up,
            crossterm::event::KeyCode::Down => Input::Down,
            crossterm::event::KeyCode::Enter => Input::Enter,
            _ => {
                continue;
            }
        };

        disable_raw_mode().unwrap();
        return input;
    }
}
