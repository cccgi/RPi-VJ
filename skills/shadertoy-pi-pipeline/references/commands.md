# Commands

Run from `E:\__Projects\RPi-VJ\engine\tools`.

Interactive prompts for a profile, pasted URLs/IDs, or a text file:

```powershell
.\run_shader_pipeline.ps1
```

Equivalent scripted inputs:

```powershell
.\run_shader_pipeline.ps1 -Profile diatribes
.\run_shader_pipeline.ps1 -Url @('https://www.shadertoy.com/view/3dyGRR','3sdGDX')
.\run_shader_pipeline.ps1 -File E:\lists\new-shaders.txt
```

The profile name or file stem becomes the batch name automatically. The
script prints it at startup. A stopped run resumes with:

```powershell
.\run_shader_pipeline.ps1 -Resume new-shaders
```

For a package and benchmark without changing the live show, add `-NoDeploy`.
The batch directory contains `ids.json`, `excluded.json`, `pi-benchmark.json`,
`benchmark-rejected.json`, `qualified.json`, and the append package.
