use async_process::Command;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
    Terminal,
};
use serde::Deserialize;
use std::{error::Error, io, time::Duration};
use tokio::{sync::mpsc, task};

#[derive(Debug, Deserialize)]
struct ContainerInfo {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Image")]
    image: String,
    #[serde(rename = "Command")]
    command: String,
    #[serde(rename = "CreatedAt")]
    created_at: String,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "Ports")]
    ports: String,
    #[serde(rename = "Names")]
    names: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> Result<(), Box<dyn Error>> {
    let (tx, mut rx) = mpsc::channel(100);
    task::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(100)).unwrap() {
                if let CEvent::Key(key) = event::read().unwrap() {
                    tx.send(key).await.unwrap();
                }
            }
        }
    });

    let commands = vec!["ps", "ps -a", "stop", "prune"];
    let mut selected_command = 0;
    let mut selected_container = 0;
    let mut containers = Vec::new();
    let mut all_flag = false;
    let mut status_message = String::new();

    loop {
        if commands[selected_command] == "ps" || commands[selected_command] == "ps -a" {
            containers = get_docker_ps_output(all_flag).await;
        }

        terminal.draw(|f| {
            let size = f.area();
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Percentage(20),
                        Constraint::Percentage(75),
                        Constraint::Percentage(5),
                    ]
                    .as_ref(),
                )
                .split(size);

            let menu_items: Vec<ListItem> = commands
                .iter()
                .enumerate()
                .map(|(i, cmd)| {
                    let style = if i == selected_command {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(cmd.to_string()).style(style)
                })
                .collect();
            let menu = List::new(menu_items)
                .block(Block::default().borders(Borders::ALL).title("Commands"));
            f.render_widget(menu, chunks[0]);

            let rows = containers.iter().enumerate().map(|(i, c)| {
                let style = if i == selected_container {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                Row::new(vec![
                    c.id.clone(),
                    c.image.clone(),
                    c.command.clone(),
                    c.status.clone(),
                    c.names.clone(),
                ])
                .style(style)
            });
            let table = Table::new(
                rows,
                &[
                    Constraint::Length(12),
                    Constraint::Length(20),
                    Constraint::Length(30),
                    Constraint::Length(20),
                    Constraint::Length(20),
                ],
            )
            .header(
                Row::new(vec!["ID", "Image", "Command", "Status", "Names"]).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Docker Containers"),
            );
            f.render_widget(table, chunks[1]);

            let status = Paragraph::new(status_message.clone())
                .style(Style::default().fg(Color::White).bg(Color::Blue));
            f.render_widget(status, chunks[2]);
        })?;

        if let Ok(key) = rx.try_recv() {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Down => {
                    if selected_container < containers.len().saturating_sub(1) {
                        selected_container += 1;
                    }
                }
                KeyCode::Up => {
                    if selected_container > 0 {
                        selected_container -= 1;
                    }
                }
                KeyCode::Right => {
                    selected_command = (selected_command + 1) % commands.len();
                }
                KeyCode::Left => {
                    if selected_command == 0 {
                        selected_command = commands.len() - 1;
                    } else {
                        selected_command -= 1;
                    }
                }
                KeyCode::Char('s') => {
                    if let Some(container) = containers.get(selected_container) {
                        let container_id = &container.id;
                        let output = Command::new("docker")
                            .arg("stop")
                            .arg(container_id)
                            .output()
                            .await;
                        match output {
                            Ok(_) => status_message = format!("Stopped container {}", container_id),
                            Err(e) => status_message = format!("Failed to stop container: {}", e),
                        }
                    }
                }
                KeyCode::Enter => match commands[selected_command] {
                    "stop" => {
                        if let Some(container) = containers.get(selected_container) {
                            let container_id = &container.id;
                            let output = Command::new("docker")
                                .arg("stop")
                                .arg(container_id)
                                .output()
                                .await;
                            match output {
                                Ok(_) => {
                                    status_message = format!("Stopped container {}", container_id)
                                }
                                Err(e) => {
                                    status_message = format!("Failed to stop container: {}", e)
                                }
                            }
                        }
                    }
                    "prune" => {
                        let output = Command::new("docker")
                            .arg("system")
                            .arg("prune")
                            .arg("-f")
                            .output()
                            .await;
                        match output {
                            Ok(_) => status_message = "System pruned".to_string(),
                            Err(e) => status_message = format!("Failed to prune system: {}", e),
                        }
                    }
                    "ps" => {
                        all_flag = false;
                        selected_container = 0;
                    }
                    "ps -a" => {
                        all_flag = true;
                        selected_container = 0;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    Ok(())
}

async fn get_docker_ps_output(all: bool) -> Vec<ContainerInfo> {
    let mut command = Command::new("docker");
    command.arg("ps");
    if all {
        command.arg("-a");
    }
    command.arg("--format").arg("{{json .}}");

    let output = command.output().await.expect("Failed to execute docker ps");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let containers: Vec<ContainerInfo> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<ContainerInfo>(line).ok())
        .collect();

    containers
}
