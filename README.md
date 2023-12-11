# godot-voice
Basic VOIP system for Godot 3.5.
Example: https://github.com/Cafezinhu/godot-voice-example/

Features:
- Audio emits signals on the server (this allows for better control over who receives the sent audio)
- Audio compression
- Audio resampling  (44100Hz to 16000Hz)
- Server mode (doesn't process audio if in server mode)
- Mute (doesn't process input audio if muted)
- Ability to pause playback and decompression
- Jitter buffer delay time configuration
