#![allow(non_snake_case, non_camel_case_types,dead_code)]

mod runpe;
mod common;
mod unhook;
mod patch;


use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use common::*;
use clap::Parser;
use clap::CommandFactory;

#[cfg(feature = "dll")]
fn parse_command_line(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current_arg = String::new();
    let mut in_quotes = false;
    let mut escaped = false;

    for c in cmd.chars() {
        match c {
            '"' if !escaped => {
                in_quotes = !in_quotes;
                // Do not include the quote character in the argument
            },
            ' ' if !in_quotes && !escaped => {
                if !current_arg.is_empty() {
                    args.push(current_arg);
                    current_arg = String::new();
                }
            },
            '\\' if !escaped => {
                escaped = true;
            },
            _ => {
                if escaped {
                    current_arg.push('\\');
                    escaped = false;
                }
                current_arg.push(c);
            }
        }
    }

    if !current_arg.is_empty() {
        args.push(current_arg);
    }

    args
}

#[cfg(feature = "dll")]
use common::dll_specific::*;

#[no_mangle]
#[cfg(feature = "dll")]
pub extern "system" fn DllMain(_hinst_dll: *mut c_void, fdw_reason: u32, _lpv_reserved: *mut c_void) -> i32 {
    if fdw_reason == 1 {  // DLL_PROCESS_ATTACH
        init_console();
    }
    1 // Return TRUE
}

#[no_mangle]
#[cfg(feature = "dll")]
pub extern "system" fn NyxInvoke(_hwnd: *mut c_void, _hinst: *mut c_void, lpszCmdLine: *const c_char, _nCmdShow: i32) {
    let result = std::panic::catch_unwind(|| {
        let c_str = unsafe { CStr::from_ptr(lpszCmdLine) };
        let command_str = c_str.to_str().expect("Invalid UTF-8 sequence");
        let args = parse_command_line(command_str);
        
        // Check if help is requested
        if args.contains(&"-h".to_string()) || args.contains(&"--help".to_string()) {
            // Display help without triggering an error
            let help_output = if args.len() > 1 && args[0] != "-h" && args[0] != "--help" {
                Cli::command().find_subcommand_mut(args[0].clone()).map(|cmd| {
                    let mut output = Vec::new();
                    cmd.write_help(&mut output).ok();
                    String::from_utf8_lossy(&output).to_string()
                }).unwrap_or_else(|| {
                    let mut output = Vec::new();
                    Cli::command().write_help(&mut output).ok();
                    String::from_utf8_lossy(&output).to_string()
                })
            } else {
                let mut output = Vec::new();
                Cli::command().write_help(&mut output).ok();
                String::from_utf8_lossy(&output).to_string()
            };
            write_to_console(get_stdout_handle(), &help_output);
            return Ok(());
        }

        let mut cli_args = vec![String::from("NyxInvoke.dll")];
        cli_args.extend(args);

        match Cli::try_parse_from(cli_args) {
            Ok(cli) => {
                match cli.mode {
                    Mode::Clr { args, base, key, iv, assembly, unencrypted} => {
                        execute_clr_mode(args, base, key, iv, assembly, unencrypted)
                    },
                    Mode::Ps { command, script } => {
                        execute_ps_mode(command, script)
                    },
                    Mode::Bof { args, base, key, iv, bof, unencrypted } => {
                        execute_bof_mode(args, base, key, iv, bof, unencrypted)
                    },
                    Mode::Pe { args, base, key, iv, pe, unencrypted } => {
                        execute_pe_mode(args, base, key, iv, pe, unencrypted)
                    },
                }
            },
            Err(e) => {
                Err(format!("Failed to parse arguments: {}", e))
            }
        }
    });

    match result {
        Ok(Ok(())) => {
            write_to_console(get_stdout_handle(), "Operation completed successfully.\n");
        },
        Ok(Err(e)) => {
            write_to_console(get_stderr_handle(), &format!("Error: {}\n", e));
        },
        Err(_) => {
            write_to_console(get_stderr_handle(), "A panic occurred in Rust code\n");
        },
    }
}