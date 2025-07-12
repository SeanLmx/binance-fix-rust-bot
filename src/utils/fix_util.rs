use tokio_util::codec::{Decoder, Encoder};
use bytes::{BytesMut, Buf};
use std::io;


pub fn build_fix_message(mut fields: Vec<String>) -> String {
    let body = fields[2..].join("\x01") + "\x01";
    fields[1] = format!("9={}", body.len());
    let mut full_msg = fields[0..2].join("\x01") + "\x01" + &body;
    let checksum = full_msg.bytes().map(|b| b as u32).sum::<u32>() % 256;
    full_msg += &format!("10={:03}\x01", checksum);
    full_msg
}

// FIX message codec that splits messages properly
pub struct FixCodec;

impl Decoder for FixCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Look for a complete FIX message
        // FIX messages start with "8=FIX" and end with "10=XXX\x01"
        let data = src.as_ref();
        
        if data.len() < 10 {
            return Ok(None); // Not enough data yet
        }

        // Find the start of a FIX message
        if let Some(start_pos) = data.windows(5).position(|window| window == b"8=FIX") {
            if start_pos > 0 {
                // Skip any junk before the FIX message
                src.advance(start_pos);
            }
            
            // Now look for the checksum field "10=XXX\x01" which marks the end
            let remaining = src.as_ref();
            for i in 0..remaining.len().saturating_sub(6) {
                if remaining[i..i+3] == [b'1', b'0', b'='] {
                    // Found checksum field, look for the end marker
                    if let Some(end_pos) = remaining[i+3..].iter().position(|&b| b == 0x01) {
                        let msg_end = i + 3 + end_pos + 1; // Include the \x01
                        let msg_bytes = src.split_to(msg_end);
                        let msg_str = String::from_utf8_lossy(&msg_bytes).to_string();
                        return Ok(Some(msg_str));
                    }
                }
            }
        }
        
        Ok(None)
    }
}

impl Encoder<String> for FixCodec {
    type Error = io::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(item.as_bytes());
        Ok(())
    }
}
