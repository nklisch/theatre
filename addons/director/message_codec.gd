## Director message codec — length-prefixed JSON wire format.
##
## Shared by daemon.gd, plugin.gd, and mock_editor_server.gd.
## Format: [4 bytes big-endian u32 length][JSON payload UTF-8]

## Try to decode one message from a read buffer.
##
## Returns a Dictionary with the decoded message if a complete message is
## available, or an empty Dictionary if more bytes are needed or the message
## is malformed. On success, removes the consumed bytes from `read_buf`.
static func try_decode(read_buf: PackedByteArray) -> Array:
	# Returns [decoded_dict_or_empty, bytes_consumed]
	if read_buf.size() < 4:
		return [{}, 0]

	# Decode big-endian u32 from the first 4 bytes.
	var msg_len: int = (read_buf[0] << 24) | (read_buf[1] << 16) | (read_buf[2] << 8) | read_buf[3]

	if msg_len == 0:
		# Consume the 4 zero bytes and return empty (malformed message).
		return [{}, 4]

	# Wait until the full message body is buffered.
	if read_buf.size() < 4 + msg_len:
		return [{}, 0]

	# Extract the JSON body.
	var msg_bytes: PackedByteArray = read_buf.slice(4, 4 + msg_len)
	var json_str = msg_bytes.get_string_from_utf8()
	var json = JSON.new()
	if json.parse(json_str) != OK:
		return [{}, 4 + msg_len]

	var data = json.get_data()
	if typeof(data) != TYPE_DICTIONARY:
		return [{}, 4 + msg_len]

	return [data, 4 + msg_len]


## Encode a Dictionary as a length-prefixed JSON message.
##
## Returns a PackedByteArray ready to send via `put_data()`.
static func encode(data: Dictionary) -> PackedByteArray:
	var json_str = JSON.stringify(data)
	var json_bytes: PackedByteArray = json_str.to_utf8_buffer()
	var msg_len = json_bytes.size()

	var result = PackedByteArray([
		(msg_len >> 24) & 0xFF,
		(msg_len >> 16) & 0xFF,
		(msg_len >> 8) & 0xFF,
		msg_len & 0xFF,
	])
	result.append_array(json_bytes)
	return result
