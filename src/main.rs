use std::fs;
use std::io;
use std::path::PathBuf;
use wav_splitter::{split_wav, SplitOptions, minutes_to_duration};

fn main() -> io::Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    let (input_file, chunk_minutes, output_prefix) = if args.len() >= 4 {
        let file = PathBuf::from(&args[1]);
        let minutes = args[2].parse::<u64>().unwrap_or(10);
        let prefix = &args[3];
        (file, minutes, prefix.clone())
    } else {
        // Default values
        let default_input = PathBuf::from("audiofile.wav");
        println!("Using default parameters:");
        println!("  Input file: {}", default_input.display());
        println!("  Chunk duration: 10 minutes");
        println!("  Output prefix: audiofile_part");
        println!("  Output folder: audio_chunks");
        println!();
        println!("To specify custom parameters, use: cargo run -- <input_file> <chunk_minutes> <output_prefix>");
        
        (default_input, 10, "audiofile_part".to_string())
    };
    
    let chunk_duration = minutes_to_duration(chunk_minutes);
    let folder_name = "audio_chunks";
    match fs::create_dir(folder_name) {
        Ok(_) => println!("Directory {} created", folder_name),
        Err(_) => println!("Directory {} already exists", folder_name),
    }
    let output_dir = PathBuf::from(folder_name);
    
    // Create split options from parameters
    let options = SplitOptions {
        input_path: &input_file,
        chunk_duration,
        output_dir: &output_dir,
        prefix: &output_prefix,
    };
    
    // Execute the split operation
    match split_wav(&options) {
        Ok(result) => {
            println!("WAV file split completed successfully!");
            println!("Created {} chunks with total duration of {:.2} minutes", 
                     result.chunk_count,
                     result.total_duration.as_secs_f64() / 60.0);
        },
        Err(e) => eprintln!("Error: {}", e),
    }
    
    Ok(())
}
