mod delete_todo;
mod edit_todo;
mod todo_list;
mod todo_selector;

use delete_todo::delete_todo;
use edit_todo::edit_todo;
use std::io::Write;
use todo_list::TodoList;

fn main() {
    /*
    TODO 앱
    해야할 일들을 보여주고 저장하고 관리하는 앱
    '해야할 일을 관리한다' = '다했다' 라고 하거나, 수정하거나, 삭제하거나.

    '해야할 일'
    - 해야할 일의 내용
    - 해야할 일의 상태 (완료/미완료)
    - 해야할 일을 언제까지 해야하는지 (있을수도, 없을수도)

    행동별로 보자
    1. 저장된/추가된 해야할 일들을 보여주는 기능
    2. 해야할 일을 추가하여 저장하는 기능
    3. 해야할 일을 수정하는 기능
        - 해야할 일의 내용을 수정하는 기능
        - 해야할 일의 상태를 수정하는 기능
        - 해야할 일의 마감일을 수정하는 기능
    4. 해야할 일을 삭제하는 기능

    우리가 만들 프로그램의 모습을 생각해보자.
    딱 켰을 때 어떠한 화면이 나와야 할까?

    ```
    ┌──────────────────────────┬────────────┐
    │              TODO        │  DeadLine  │
    ├──────────────────────────┼────────────│
    │ Eat Food                 │ 2023-10-05 │
    ├──────────────────────────┼────────────│
    │ Sleep                    │    N/A     │
    └──────────────────────────┴────────────┘
    Choose an action:
        1) Add a TODO
        2) Edit a TODO
        3) Delete a TODO
    ```

    // Add a Todo를 눌렀을 때 어떤 화면이 나와야하는가?
    ```
    # Add a TODO
    - TODO: {...}
    - DeadLine (enter 'n' for no deadline or 'yyyy-mm-dd'): {...}

    Success!
    ```

    Q. `Edit a Todo`는 어떻게 해야합니까?
    TODO를 선택하고, TODO의 무엇을 고칠지를 선택하고, 그것을 수정해야합니다.

    Q. TODO 리스트에서 TODO 선택은 어떻게 합니까?
    가장 만만한게 번호를 입력받는 것입니다.
    근데 그것은 별로 멋진 방법은 아닙니다.
    가장 멋진 방법은 키보드 방향키를 이용하는 것입니다.
    위 아래 향키로 이동하고, 엔터를 누르면 그것을 선택하는 것입니다.

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

    Q. TODO를 선택한 후에는 어떻게 수정하게 할 것인가?
    AWS CLI처럼, 각 항목에 대해서 바꿀건지 안바꿀건지 물어보는 식으로 합시다.

    ```
    # Edit a TODO
    # Press Enter without typing anything to skip editing.
    TODO [Eat Food]: {...}
    (enter 'n' for no deadline or 'yyyy-mm-dd')
    DeadLine [2023-10-05]: {...}

    Success!
    ```

    Delete는 위 Edit 선택기로 선택한거 지우고 Success! 띄우면 됩니다.

    */

    let mut todo_list = TodoList::init(); // TODO: DB에 저장하기

    loop {
        print_todo_list(&todo_list);
        let action: Action = print_action_menu();

        match action {
            Action::Add => {
                add_new_todo(&mut todo_list);
            }
            Action::Edit => {
                edit_todo(&mut todo_list);
            }
            Action::Delete => {
                delete_todo(&mut todo_list);
            }
        }
    }
}

fn add_new_todo(todo_list: &mut TodoList) {
    println!("# Add a TODO");
    print!("TODO: ");
    std::io::stdout().flush().unwrap();

    let mut todo = String::new();
    std::io::stdin().read_line(&mut todo).unwrap();

    print!("DeadLine (enter 'n' for no deadline or 'yyyy-mm-dd'): ");
    std::io::stdout().flush().unwrap();

    // TODO: 날짜가 제대로 형식에 맞는지 확인해야함.
    let mut deadline = String::new();
    std::io::stdin().read_line(&mut deadline).unwrap();

    todo_list.push(Todo {
        content: todo.trim().to_string(),
        deadline: if deadline.trim() == "n" {
            None
        } else {
            Some(deadline.trim().to_string())
        },
    });

    println!("Success!");
}

fn print_action_menu() -> Action {
    println!(
        "Choose an action:
    1) Add a TODO
    2) Edit a TODO
    3) Delete a TODO
"
    );

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    match input.trim() {
        "1" => Action::Add,
        "2" => Action::Edit,
        "3" => Action::Delete,
        _ => {
            println!("Invalid input! Try again.");
            print_action_menu()
        }
    }
}

fn print_todo_list(todo_list: &TodoList) {
    // TODO: todo list를 가져와야함.
    println!(
        "┌──────────────────────────┬────────────┐
│              TODO        │  DeadLine  │"
    );

    for todo in todo_list.iter() {
        println!("├──────────────────────────┼────────────│");
        let deadline = match &todo.deadline {
            Some(deadline) => deadline,
            None => "N/A",
        };
        print!("│ {: <24} ", todo.content);
        println!("│ {: <10} │", deadline);
    }

    println!("└──────────────────────────┴────────────┘");
}

enum Action {
    Add,
    Edit,
    Delete,
}

struct Todo {
    pub content: String,
    pub deadline: Option<String>,
}
