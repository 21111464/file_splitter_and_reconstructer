use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::exit;

fn list_prompt(prompt: &str, options: &BTreeMap<String, &str>) -> String {
    loop {
        println!("{}", prompt);
        for (i, (option, option_type)) in options.iter().enumerate() {
            match *option_type {
                "directory" => println!("{}. {}", i + 1, option),
                "chunk" => println!("\x1b[92m{}. {}\x1b[0m", i + 1, option),
                "action" => println!("\x1b[94m{}. {}\x1b[0m", i + 1, option),
                "exit" => println!("\x1b[91m{}. {}\x1b[0m", i + 1, option),
                _ => println!("{}. {}", i + 1, option),
            }
        }
        print!("Enter the number of your choice: ");
        io::stdout().flush().unwrap();
        let mut response = String::new();
        io::stdin().read_line(&mut response).unwrap();
        if let Ok(index) = response.trim().parse::<usize>() {
            if index > 0 && index <= options.len() {
                return options.keys().nth(index - 1).unwrap().clone();
            }
        }
        println!("Invalid choice. Please select a valid number from the list.");
    }
}

fn reconstruct_file(directory: &Path) -> io::Result<String> {
    let info_path = directory.join("info.json");
    let name = if info_path.exists() {
        let data = fs::read_to_string(&info_path)?;
        serde_json::from_str::<serde_json::Value>(&data)
            .ok()
            .and_then(|v| {
                v.get("original_filename")
                    .and_then(|n| n.as_str().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "reconstructed_file".to_string())
    } else {
        "reconstructed_file".to_string()
    };

    let output_path = directory.join(&name);
    let mut output_file = BufWriter::new(File::create(&output_path)?);

    // Collect and sort chunk files
    let mut chunk_files: Vec<_> = fs::read_dir(directory)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("chunk"))
        .map(|e| e.path())
        .collect();

    chunk_files.sort();

    // Concatenate all chunks
    for chunk_path in chunk_files {
        let mut chunk_file = BufReader::new(File::open(&chunk_path)?);
        io::copy(&mut chunk_file, &mut output_file)?;
    }

    Ok(name)
}

fn split_file(input_path: &Path, savedir: &Path) -> io::Result<()> {
    const CHUNK_SIZE: usize = 5 * 1024 * 1024; // 5MB

    // Create directory if it doesn't exist
    if !savedir.exists() {
        fs::create_dir_all(savedir)?;
    }

    // Check if directory is empty
    if fs::read_dir(savedir)?.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Directory is not empty",
        ));
    }

    // Save original filename info
    let info = serde_json::json!({
        "original_filename": input_path.file_name().unwrap().to_string_lossy()
    });
    fs::write(
        savedir.join("info.json"),
        serde_json::to_string(&info).unwrap(),
    )?;

    // Split file into chunks
    let mut input_file = BufReader::new(File::open(input_path)?);
    let mut buffer = vec![0u8; CHUNK_SIZE];
    let mut chunk_index = 0;

    loop {
        let bytes_read = input_file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        let chunk_name = format!("chunk{:03}", chunk_index);
        let chunk_path = savedir.join(&chunk_name);
        let mut chunk_file = BufWriter::new(File::create(&chunk_path)?);
        chunk_file.write_all(&buffer[..bytes_read])?;

        chunk_index += 1;
    }

    Ok(())
}

fn main() {
    let mut directory = env::current_dir().unwrap();
    let mut options = BTreeMap::new();
    options.insert("Reconstruct file".to_string(), "action");
    options.insert("Split file".to_string(), "action");
    options.insert("Exit".to_string(), "exit");
    let choice = list_prompt("Reconstruct or split file:", &options);

    match choice.as_str() {
        "Reconstruct file" => loop {
            let mut dir_options = BTreeMap::new();
            if let Ok(entries) = fs::read_dir(&directory) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            dir_options.insert(name.to_string(), "directory");
                        }
                    }
                }
            }
            dir_options.insert("Reconstruct".to_string(), "action");
            dir_options.insert("Exit".to_string(), "exit");
            println!("\n>>>\t{}", directory.display());
            let chunk_files: Vec<_> = fs::read_dir(&directory)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("chunk"))
                .collect();
            if !chunk_files.is_empty() {
                println!(
                    "\tFound {} chunk files in this directory.",
                    chunk_files.len()
                );
            } else {
                println!("\tNo chunk files found in this directory.");
            }
            let choice = list_prompt("", &dir_options);
            match choice.as_str() {
                "Reconstruct" => {
                    match reconstruct_file(&directory) {
                        Ok(name) => {
                            println!("Reconstructed file saved as \"{}\".", name);
                        }
                        Err(e) => {
                            println!("Error during reconstruction: {}", e);
                        }
                    }
                    break;
                }
                "Exit" => exit(0),
                other => {
                    directory = directory.join(other);
                }
            }
        },
        "Split file" => {
            print!("Enter the path to the file to split\n>>> ");
            io::stdout().flush().unwrap();
            let mut input_path = String::new();
            io::stdin().read_line(&mut input_path).unwrap();
            let input_path = input_path.trim();

            if !Path::new(input_path).is_file() {
                println!("File does not exist.");
                exit(1);
            }

            print!("Give a directory to save the chunks\n>>> ");
            io::stdout().flush().unwrap();
            let mut savedir = String::new();
            io::stdin().read_line(&mut savedir).unwrap();
            let savedir = savedir.trim();

            match split_file(Path::new(input_path), Path::new(savedir)) {
                Ok(_) => {
                    println!("File split successfully.");
                }
                Err(e) => {
                    println!("Error during splitting: {}", e);
                    exit(1);
                }
            }
        }
        "Exit" => exit(0),
        _ => {}
    }
}
