module "module-1" {
    source = "./modules/timer"
    
    duration = "1"
}

module "module-2" {
    source = "./modules/timer"
    
    duration = "2"
}