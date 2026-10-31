use std::env;
use std::process;

mod apitranslator;
mod dosallocmem;
mod dosallocsharedmem;
mod doscreatethread;
mod dosgetgivesharedmem;
mod dosgetnamedsharedmem;
mod fstrans;
mod mmu;
mod pmserver;
mod winsendmsg;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "--daemon" | "pmserver" => {
            println!("Starting Neo OS/2 Subsystem Server...");
            pmserver::run_registry_daemon();
        }
        
        "run" => {
            if args.len() < 3 {
                println!("Error: Missing legacy executable path.");
                process::exit(1);
            }
            let legacy_path = &args[2];
            println!("Loading OS/2 Executable: {}", legacy_path);
            
            // Route to your specific CPU emulator module (ensure the boot function exists in your target file)
            apitranslator::boot_legacy_executable(legacy_path);
        }
        
        _ => {
            println!("Unknown command: {}", args[1]);
            print_usage();
            process::exit(1);
        }
    }
}

fn print_usage() {
    println!("Neo OS/2 Unified Binary");
    println!("Usage:");
    println!("  neo_os2 --daemon         Start the central PM and Memory server");
    println!("  neo_os2 run <path.exe>   Run a legacy 16/32-bit OS/2 application");
}