use std::fs::{self, File};
use std::io::{self, Write, BufWriter, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::codecs::CodecParameters;

/// Information about an audio chunk
pub struct ChunkInfo {
    /// Start time of the chunk
    pub start_time: Duration,
    /// End time of the chunk
    pub end_time: Duration,
    packets: Vec<usize>, // Indices of packets in the global packets list
}

/// Configuration options for WAV splitting
pub struct SplitOptions<'a> {
    /// Path to the input WAV file
    pub input_path: &'a Path,
    /// Desired duration for each chunk
    pub chunk_duration: Duration,
    /// Directory where output files will be saved
    pub output_dir: &'a Path,
    /// Prefix for output filenames
    pub prefix: &'a str,
}

/// Result of WAV splitting operation
pub struct SplitResult {
    /// Total number of chunks created
    pub chunk_count: usize,
    /// Total duration of the input file
    pub total_duration: Duration,
    /// Paths to generated output files
    pub output_files: Vec<PathBuf>,
}

/// Split a WAV file into chunks of specified duration
///
/// # Arguments
/// * `options` - The configuration options for the splitting process
///
/// # Returns
/// * `Result<SplitResult, io::Error>` - The result of the splitting operation
///
/// # Example
/// ```no_run
/// use wav_splitter::{SplitOptions, split_wav};
/// use std::path::Path;
/// use std::time::Duration;
///
/// let options = SplitOptions {
///     input_path: Path::new("input.wav"),
///     chunk_duration: Duration::from_secs(600), // 10 minutes
///     output_dir: Path::new("chunks"),
///     prefix: "track",
/// };
///
/// match split_wav(&options) {
///     Ok(result) => println!("Split into {} chunks", result.chunk_count),
///     Err(e) => eprintln!("Error: {}", e),
/// }
/// ```
pub fn split_wav(options: &SplitOptions) -> io::Result<SplitResult> {
    println!("Processing file: {}", options.input_path.display());
    println!("Target chunk duration: {} seconds ({} minutes)", 
        options.chunk_duration.as_secs(), 
        options.chunk_duration.as_secs() / 60);
    
    // Create output directory if it doesn't exist
    if !options.output_dir.exists() {
        fs::create_dir_all(options.output_dir)?;
    }
    
    // Open the media source
    let file = Box::new(ReadOnlySource::new(File::open(options.input_path)?));
    let mss = MediaSourceStream::new(file, Default::default());
    
    // Create a hint to help with format detection
    let mut hint = Hint::new();
    hint.with_extension("wav");
    
    // Use default options
    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    
    // Probe the format
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Error probing format: {}", e)))?;
    
    let mut format = probed.format;
    
    // Get the default track
    let track = format.default_track()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No default track found"))?;
    
    // Get codec parameters and time base
    let codec_params = track.codec_params.clone();
    let time_base = codec_params.time_base
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No time base found"))?;
    
    // Store all packets and their durations
    let mut packets = Vec::new();
    let mut packet_times = Vec::new();
    let mut total_duration = Duration::from_secs(0);
    
    // First pass: read all packets and calculate timestamps
    println!("First pass: reading packets and calculating timestamps...");
    while let Ok(packet) = format.next_packet() {
        // Calculate duration of this packet
        let frame_len = packet.dur;
        let packet_duration = Duration::from_secs_f64(
            frame_len as f64 * time_base.numer as f64 / time_base.denom as f64
        );
        
        total_duration += packet_duration;
        packet_times.push(total_duration);
        packets.push(packet);
    }
    
    if packets.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "No audio packets found"));
    }
    
    println!("Found {} packets, total duration: {:.2} seconds ({:.2} minutes)", 
        packets.len(), 
        total_duration.as_secs_f64(),
        total_duration.as_secs_f64() / 60.0
    );
    
    // Second pass: determine chunk boundaries
    println!("Second pass: determining chunk boundaries...");
    let mut chunks = Vec::new();
    let mut chunk_start_packet = 0;
    let mut chunk_start_time = Duration::from_secs(0);
    
    while chunk_start_packet < packets.len() {
        // Find the packet that would end this chunk
        let target_end_time = chunk_start_time + options.chunk_duration;
        
        // Find the packet index that's closest to our target end time
        let mut chunk_end_packet = chunk_start_packet;
        while chunk_end_packet < packets.len() && 
              (chunk_end_packet == chunk_start_packet || 
               packet_times[chunk_end_packet - 1] < target_end_time) {
            chunk_end_packet += 1;
        }
        
        // Ensure we include at least one packet
        if chunk_end_packet == chunk_start_packet {
            chunk_end_packet = chunk_start_packet + 1;
        }
        
        // Get the actual end time for this chunk
        let chunk_end_time = if chunk_end_packet < packets.len() {
            packet_times[chunk_end_packet - 1]
        } else {
            total_duration
        };
        
        // Create packet index list for this chunk
        let mut chunk_packets = Vec::new();
        for i in chunk_start_packet..chunk_end_packet {
            chunk_packets.push(i);
        }
        
        chunks.push(ChunkInfo {
            start_time: chunk_start_time,
            end_time: chunk_end_time,
            packets: chunk_packets,
        });
        
        // Move to next chunk
        chunk_start_packet = chunk_end_packet;
        chunk_start_time = chunk_end_time;
        
        // Break if we've processed all packets
        if chunk_start_packet >= packets.len() {
            break;
        }
    }
    
    println!("Splitting into {} chunks:", chunks.len());
    
    // Debug output to check chunk durations
    for (i, chunk) in chunks.iter().enumerate() {
        let duration = (chunk.end_time - chunk.start_time).as_secs_f64();
        println!("Chunk {} duration: {:.2} minutes ({:.2} seconds), packets: {}", 
            i+1, duration/60.0, duration, chunk.packets.len());
    }
    
    // Read the WAV header from the original file to use as a template
    let mut original_file = File::open(options.input_path)?;
    let mut header_buf = Vec::new();
    
    // Read first 44 bytes (standard WAV header)
    let header_size = 44;
    original_file.seek(SeekFrom::Start(0))?;
    let bytes_read = io::Read::take(&mut original_file, header_size as u64)
        .read_to_end(&mut header_buf)?;
    
    if bytes_read < header_size {
        return Err(io::Error::new(io::ErrorKind::Other, "Failed to read WAV header"));
    }
    
    // Store output file paths
    let mut output_files = Vec::with_capacity(chunks.len());
    
    // Get sample rate and other parameters to calculate correct WAV header for each chunk
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params.channels.unwrap_or(symphonia::core::audio::Channels::FRONT_LEFT | symphonia::core::audio::Channels::FRONT_RIGHT).count();
    let bits_per_sample = match codec_params.bits_per_sample {
        Some(bits) => bits as u16,
        None => 16, // Default to 16-bit
    };
    let bytes_per_sample = (bits_per_sample / 8) as u16;
    
    // Third pass: write chunks to files
    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let output_filename = format!("{}_{:03}.wav", options.prefix, chunk_idx + 1);
        let output_path = options.output_dir.join(&output_filename);
        output_files.push(output_path.clone());
        
        println!(
            "Writing chunk {}/{}: {} (duration: {:.2} minutes, {} packets)",
            chunk_idx + 1,
            chunks.len(),
            output_filename,
            (chunk.end_time - chunk.start_time).as_secs_f64() / 60.0,
            chunk.packets.len()
        );
        
        let mut output = BufWriter::new(File::create(&output_path)?);
        
        // Calculate chunk data size
        let mut chunk_data_size: u32 = 0;
        for &packet_idx in &chunk.packets {
            chunk_data_size += packets[packet_idx].data.len() as u32;
        }
        
        // Write WAV header
        write_wav_header(&mut output, chunk_data_size, sample_rate, channels as u16, bits_per_sample, bytes_per_sample)?;
        
        // Write all packets for this chunk
        for &packet_idx in &chunk.packets {
            output.write_all(&packets[packet_idx].data)?;
        }
        output.flush()?;
    }
    
    println!("Successfully split WAV file into {} chunks in directory: {}", 
        chunks.len(), options.output_dir.display());
    
    Ok(SplitResult {
        chunk_count: chunks.len(),
        total_duration,
        output_files,
    })
}

/// Write a proper WAV header to the output file
fn write_wav_header(
    writer: &mut impl Write,
    data_size: u32,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    bytes_per_sample: u16
) -> io::Result<()> {
    // Calculate important values
    let byte_rate = sample_rate * (channels as u32) * (bytes_per_sample as u32);
    let block_align = channels * bytes_per_sample;
    let file_size = data_size + 36; // 36 + data_size
    
    // RIFF header
    writer.write_all(b"RIFF")?;
    writer.write_all(&file_size.to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    
    // fmt chunk
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // Chunk size (16 for PCM)
    writer.write_all(&1u16.to_le_bytes())?;  // Audio format (1 = PCM)
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;
    
    // data chunk
    writer.write_all(b"data")?;
    writer.write_all(&data_size.to_le_bytes())?;
    
    Ok(())
}

/// Utility function to convert minutes to Duration
pub fn minutes_to_duration(minutes: u64) -> Duration {
    Duration::from_secs(minutes * 60)
}