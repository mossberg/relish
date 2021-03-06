use std::io;
use std::io::{stdin, Read, BufReader,BufRead};
use std::env;
use std::fs::File;
use std::path::{PathBuf};
use std::process::{Stdio, Command, exit};

// trait methods
use std::io::Write; // flush stdout

static BUILTINS: [&'static str; 3] = ["exit", "cd", "pwd"];

struct CommandLine {
    // command name
    cmd: String,
    // arguments, starting directly with the first actual argument
    args: Vec<String>,
    // whether to execute as background process
    bg: bool
}

// probaby doesn't need to be a macro
macro_rules! printerr {
    ($msg:expr) => (println!("relish: {}", $msg));
}


/*
 * get_pwd - returns String containing pwd or "???" if there was an error
 */
fn get_pwd() -> String {
    match env::current_dir() {
        Ok(pwd) => {
            pwd.to_string_lossy().to_string()
        }
        Err(_) => {
            "???".to_string()
        }
    }
}


/*
 * execute - execute shell command line based on input CommandLine
 */
fn execute(cmdline: &CommandLine) {
    let mut _cmd = Command::new(&cmdline.cmd);
    let cmd = _cmd.args(&cmdline.args);

    if cmdline.bg {
        if let Err(why) = cmd.spawn() {
            printerr!(why);
        }
    } else {
        if let Err(why) = cmd.stdout(Stdio::inherit())
                             .stderr(Stdio::inherit())
                             .output() {
            if let Some(errno) = why.raw_os_error() {
                match errno {
                    2 => printerr!(format!("{}: {}", cmdline.cmd,
                                           "Command not found")),
                    _  => printerr!(format!("{}: {}", cmdline.cmd, why)),
                }
                return
            }

            // getting here is pretty rare, means error didn't have os errno
            printerr!(why);
        }
    }
}


/*
 * builtin - implement shell builtins
 */
fn builtin(cmdline: &CommandLine) {
    match &cmdline.cmd[..] {
        "exit" => {
            println!("So long, and thanks for all the fish!");
            exit(0);
        }
        "cd" => {
            // get dir to change to based on the length of cmdline.args
            let dir = if cmdline.args.len() == 0 {
                env::home_dir().unwrap_or(PathBuf::from("."))
            } else {
                // if they say `cd -`
                if cmdline.args[0] == "-" {
                    // return $OLDPWD, or "." if it's not available
                    PathBuf::from(&env::var("OLDPWD")
                                             .unwrap_or(".".to_string()))
                } else {
                    // create PathBuf from what they actually said
                    PathBuf::from(&cmdline.args[0])
                }
            };

            // set $OLDPWD
            env::set_var("OLDPWD", &env::current_dir()
                                        .unwrap_or(PathBuf::from(".".to_string())));
            // change directory
            if let Err(why) = env::set_current_dir(&dir) {
                printerr!(why);
            }
        }
        "pwd" => println!("{}", get_pwd()),
        _ => {}
    }
}

fn print_prompt() -> io::Result<()> {
    print!("{}", get_prompt());
    io::stdout().flush()
}

/*
 * get_prompt - generate and return shell prompt
 */
fn get_prompt() -> String {
    // get username
    let username = env::var("USER").unwrap_or("???".to_string());

    // get hostname
    let hostname = Command::new("/bin/hostname").output().unwrap();
    let hostname = String::from_utf8_lossy(&hostname.stdout);
    let hostname = hostname.trim();

    format!("{}@{} {} $ ", username, hostname, get_pwd())
}


/*
 * preprocess - main parsing routine responsible for populating CommandLine
 * struct
 */
fn preprocess(cmdline: &mut CommandLine) {
    let tmp = cmdline.cmd.clone();
    // TODO: this is awful, refactor to not use a loop
    for (i, each) in tmp.split(' ').enumerate() {

        if each.trim() == "" {
            // eat extra tabs/spaces
            continue;
        } else if each.trim().chars().nth(0).unwrap() == '#' {
            // stop parsing if there's a comment
            // ok to use unwrap because we've guaranteed input isn't empty
            break;
        } else if each.trim().chars().nth(0).unwrap() == '&'  {
            // background process, ignoring rest of input
            // TODO: probably shouldn't just ignore rest of input. also,
            // the & has to have a space before it right now

            if cmdline.bg == true {
                printerr!("background symbol `&` specified twice");
            }
            cmdline.bg = true;
            break;
        } else {
            // ok, this is real input

            let mut tmp = each.trim().to_string();

            // did they not include a space before the bg symbol?
            if each.trim().chars().nth(each.len()-1).unwrap() == '&' {
                if cmdline.bg == true {
                    printerr!("background symbol `&` specified twice");
                }
                cmdline.bg = true;
                tmp.pop();
            }
        

            // if it's the very first split word
            if i == 0 {
                cmdline.cmd = tmp;
                // handle `exit` builtin so we can handle it ASAP
                if each.trim() == "exit" {
                    builtin(&cmdline);
                }
            } else {
                // regular argument
                cmdline.args.push(tmp);
            }
        }
    }
}


fn main() {
    if env::args().count() > 2 {
        printerr!(format!("Usage: {} [script]", env::args().nth(0).unwrap()));
        exit(1);
    }

    let reader: Box<BufRead> = if env::args().count() == 2 {
        let path = &env::args().nth(1).unwrap();
        let file = match File::open(path) {
            Ok(file) => file,
            Err(_) => {
                printerr!(format!("couldn't open {}", path));
                exit(1);
            }
        };
        Box::new(BufReader::new(file))
    } else {
        let i = io::stdin();
        Box::new(BufReader::new(i))
    };

    if let Err(why) = print_prompt() {
        printerr!(why);
    }

    // main shell loop
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => {
                exit(0);
            }
        };

        let mut cmdline: CommandLine = CommandLine {
            cmd: line,
            args: Vec::new(),
            bg: false
        };


        // check if blank/comment
        cmdline.cmd = cmdline.cmd.trim().to_string();
        match cmdline.cmd.chars().nth(0) {
            Some(first) =>
                if first == '#' {
                    continue
                },
            None => continue // empty string
        }

        // parse
        preprocess(&mut cmdline);

        // handle builtins
        // NOTE: `exit` is handled in preprocess() for efficiency
        if BUILTINS.contains(&&cmdline.cmd[..]) {
            builtin(&cmdline)
        } else {
            execute(&cmdline);
        }

        if let Err(why) = print_prompt() {
            printerr!(why);
            continue;
        }
    }
}
