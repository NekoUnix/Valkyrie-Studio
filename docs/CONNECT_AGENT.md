# Connect an AI agent to Valkyrie Studio

If you want to see the connection before setting it up, [watch the full-screen walkthrough](../media/Valkyrie-Studio-Walkthrough-Public.mp4). Vaelari moves under local agent commands and speaks with ElevenLabs while the Inputs panel shows **READY**.

An agent controls the **loaded Live2D model** through Valkyrie Studio's local API. The
agent does not connect directly to a `.moc3` file or need any renderer SDK itself.

1. Start Valkyrie Studio and load a `.model3.json` in **Scene**. You should see the
   model on the canvas.
2. Open **Inputs → Connect an AI agent** and click **Enable agent control**. The
   indicator should say **READY**. Phone and webcam modes take control away from
   the agent while selected.
3. Open PowerShell in the Valkyrie Studio project folder. Run
   `ruby bin/agent --check` to verify the connection. `ruby bin/agent --connect`
   selects agent mode and checks it in one step. The CLI automatically reads the
   private session token from `tmp/api-token`.
4. Test a visible command:

   ```powershell
   ruby bin/agent '{"op":"emotion","name":"joy","duration":3}'
   ```

5. To make **your own AI agent** control the model, give it this guide and
   [the API reference](API.md). It can invoke `ruby bin/agent 'JSON_COMMAND'` as
   a local tool, or open a TCP socket to `127.0.0.1:4141`. Each request is a
   single UTF-8 JSON object followed by `\n`; include `token` from
   `tmp/api-token`. Read one JSON response per request and check `ok`.

The token is a local secret, changes on restart unless explicitly configured,
and should never be pasted into a prompt, public log, or stream. Keep the API
bound to loopback. The agent provider's own API key stays in that provider's
process; Valkyrie Studio only receives movement and speech commands.

The commands above assume Ruby is on your PATH. On the development workstation, use
the Ruby executable under `.tools/rubyinstaller-3.4.11-1-x64/bin/` if needed. A fresh
checkout must [install Ruby and build the bridge](BUILD.md) first.

## Commands the agent can use

```powershell
ruby bin/agent '{"op":"emotion","name":"thinking","duration":3}'
ruby bin/agent '{"op":"tracking","values":{"yaw":16,"pitch":-5,"roll":3,"eyeBlinkLeft":0,"eyeBlinkRight":0}}'
ruby bin/agent '{"op":"tts","provider":"elevenlabs","text":"Hello from Valkyrie Studio!","autoplay":true}'
```

For smooth movement, maintain one socket and send `tracking` updates about
20–30 times per second. Tracking expires after half a second without updates;
expressions and direct parameter overrides have explicit durations. Use
`ruby examples/agent_performance.rb` for a complete, editable control example.
`schema` lists the loaded model's actual parameter IDs and ranges before using
`parameters`; those IDs vary by model.

For multiple voice lines, synchronized head motion, and a finished MP4, use
the [agent performance workflow](AGENT_PERFORMANCE.md). It measures every
generated line before placing motion and pauses, including long scripts.

ElevenLabs speech requires a configured account key and selected voice in the
**Audio & Speech** panel. Choose from **Your ElevenLabs voices**, then use
**Generate speech** in the UI or the `tts` command above. Speech generation is
asynchronous; `status.elevenlabs.busy` and `status.audio` show progress and the
loaded clip. With autoplay, the model lip-syncs during playback. Do not give an
AI agent the ElevenLabs key; Valkyrie Studio uses its locally saved credential.

If `--check` fails, make sure the studio is running, the model is loaded, and
the API port in `config.yml` matches `L2D_API_PORT` if you changed it. If the
model does not move, check **Inputs** is in `agent` mode and that your tracking
stream has not expired. See [API.md](API.md) for every supported command.
