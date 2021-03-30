use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::time::Duration;

use terraform::{Terraform, Error, TerraformResourceStatus};

fn main() -> Result<(), Error> {
    let (sender, receiver) = channel();

    let environment = HashMap::new();

    let stdout_reader = std::thread::spawn(|| {
        let terraform = Terraform::new(
            "terraform",
            "./terraform",
            environment,
            Duration::from_secs(600),
            sender,
        )
        .unwrap();

        terraform.run_init().unwrap();
        terraform.run_plan("output.plan").unwrap();
        terraform.run_apply("output.plan").unwrap();
        terraform.run_destroy().unwrap();
    });

    let mut plan_modifications = 0;
    let mut apply_running = 0;
    let mut apply_done = 0;
    let mut destroy_running = 0;
    let mut destroy_done = 0;

    while let Ok(event) = receiver.recv() {
        if event.command == "plan" && event.status == Some(TerraformResourceStatus::Completed) {
            plan_modifications = event.create_count.unwrap_or(0) + event.update_count.unwrap_or(0) + event.delete_count.unwrap_or(0)
        } else if event.command == "apply" {
            if event.status == Some(TerraformResourceStatus::Started) {
                apply_running += 1;
                println!("apply - total: {} | running: {} | done: {}", plan_modifications, apply_running, apply_done);
            } else if event.status == Some(TerraformResourceStatus::Done) {
                apply_running -= 1;
                apply_done += 1;
                println!("apply - total: {} | running: {} | done: {}", plan_modifications, apply_running, apply_done);
            }
        } else if event.command == "destroy" {
            if event.status == Some(TerraformResourceStatus::Started) {
                destroy_running += 1;
                println!("destroy - total: {} | running: {} | done: {}", apply_done, destroy_running, destroy_done);
            } else if event.status == Some(TerraformResourceStatus::Done) {
                destroy_running -= 1;
                destroy_done += 1;
                println!("destroy - total: {} | running: {} | done: {}", apply_done, destroy_running, destroy_done);
            }
        }
    }

    let _ = stdout_reader.join();

    Ok(())
}
