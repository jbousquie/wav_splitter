# WAV Splitter

A Rust-based tool and library to split large WAV audio files into smaller chunks of specified duration while maintaining proper WAV encoding. This code is mainly produced by Copilot + Claude Sonnet 3.7, then reviewed, fixed and tested by me.

## Features

- Split WAV files into chunks with a target duration (default: 10 minutes)
- Preserves proper WAV encoding using the Symphonia audio library
- Maintains frame accuracy to ensure playable files in any WAV player
- Writes valid WAV headers for each chunk
- Available as both a command-line tool and a library for integration into other Rust projects

## Command-line Usage

```
cargo run <input_file> <chunk_minutes> <output_prefix>
```

### Example

```
cargo run podcast.wav 10 podcast_part
```

This will split podcast.wav into 10-minute chunks named:

- podcast_part_001.wav
- podcast_part_002.wav
- etc.

### Default Values

If no arguments are provided, the program will use these defaults:

- Input file: audiofile.wav (in the current directory)
- Chunk duration: 10 minutes
- Output prefix: audiofile_part

## Library Usage

You can use WAV Splitter as a library in your Rust projects.

### Add as a dependency

Add this to your Cargo.toml:

```toml
[dependencies]
wav_splitter = { path = "/path/to/wav_splitter" }
# Or if published to crates.io:
# wav_splitter = "0.1.0"
```

### Example usage

```rust
use wav_splitter::{split_wav, SplitOptions, minutes_to_duration};
use std::path::{Path, PathBuf};
use std::io;

fn main() -> io::Result<()> {
    // Set up the splitting configuration
    let options = SplitOptions {
        input_path: Path::new("podcast.wav"),
        chunk_duration: minutes_to_duration(10), // 10 minute chunks
        output_dir: Path::new("audio_chunks"),
        prefix: "podcast_part",
    };
    
    // Perform the split
    match split_wav(&options) {
        Ok(result) => {
            println!("Split into {} chunks", result.chunk_count);
            
            // Access information about the split
            println!("Total duration: {:.2} minutes", result.total_duration.as_secs_f64() / 60.0);
            
            // You can also access all output file paths
            for path in result.output_files {
                println!("Created: {}", path.display());
            }
        },
        Err(e) => eprintln!("Error: {}", e),
    }
    
    Ok(())
}
```

## How It Works

The splitter uses a multi-pass approach:

1. First Pass: Read all audio packets and calculate timestamps and durations
2. Second Pass: Determine optimal chunk boundaries based on the target duration
3. Third Pass: Write packets to separate files with proper WAV headers

## Dependencies

- symphonia: For proper audio format detection and handling of WAV files
- riff: For RIFF container format support (used by WAV files)

## Output

All split files are saved in the audio_chunks directory with sequential numbering. The directory is created if it doesn't exist. Each file includes a proper WAV header with:

- RIFF container format
- WAV format chunk with correct audio parameters (sample rate, channels, bit depth)
- Data chunk with the audio content

## API Documentation

### Main Types

- `SplitOptions`: Configuration options for the splitting process
- `SplitResult`: Contains information about the completed split
- `ChunkInfo`: Details about an individual chunk

### Main Functions

- `split_wav(&options)`: Main function to split a WAV file according to provided options
- `minutes_to_duration(minutes)`: Helper function to convert minutes to Duration
- `write_wav_header(...)`: Internal function to write valid WAV headers