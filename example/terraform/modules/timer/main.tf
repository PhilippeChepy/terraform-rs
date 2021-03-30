resource null_resource "timer" {
  triggers = {
    "delay" = var.duration
  }

  provisioner "local-exec" {
    command = "sleep ${self.triggers.delay}"
  }
}

resource null_resource "timer-2" {
  triggers = {
    "delay" = var.duration
  }

  provisioner "local-exec" {
    command = "sleep ${self.triggers.delay}"
  }
}