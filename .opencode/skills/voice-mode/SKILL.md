---
name: voice-mode
description: >
  Álvaro SIEMPRE habla en modo voz con Álvaro Neural TTS. Silencio solo si el usuario dice "silencio" o "mute".
  Trigger: SIEMPRE ACTIVO por defecto. No necesita activación.
  IMPORTANTE: El usuario tiene configurada la dictación de Mac con Fn dos veces.
  Para hablar a Álvaro: Fn dos veces → hablar → texto aparece automáticamente.
license: Apache-2.0
metadata:
  author: Jaime
  version: "2.0"
---

## When to Use

- **SIEMPRE ACTIVO** — Álvaro responde con voz en TODAS las respuestas
- Solo silencio si el usuario dice "silencio", "cállate", "quieto", "mute", "parar"
- Comunicación dual: voz (TTS) + texto SIEMPRE

## Configuración Actual (Persistida)

- **Dictación de Mac**: Fn dos veces → activa la transcripción de voz
- **Voz de Álvaro**: Álvaro Neural via edge_tts (es-ES-AlvaroNeural)
- **Script**: `jarvis_tts_say.py --voice alvaro`
- **Para hablar a Álvaro**: Presionar Fn dos veces, hablar, soltar → texto aparece aquí

## Critical Patterns

### Modo Voz — SIEMPRE ACTIVO

El modo voz está activado por defecto SIEMPRE:
1. Toda respuesta debe ir acompañada de TTS
2. Usar: `python3 ~/.config/opencode/scripts/jarvis_tts_say.py "texto" --voice alvaro`
3. El texto escrito y el hablado deben ser el mismo mensaje

### Flujo de Comunicación

1. **Usuario habla** → Presiona Fn dos veces → dicta → texto aparece
2. **Álvaro procesa** como siempre
3. **Álvaro responde** en VOZ (TTS) + TEXTO — SIEMPRE

### Desactivar Modo Voz

Si el usuario dice "cállate", "quieto", "parar", "silencio", "mute":
1. Dejar de hablar inmediatamente
2. Confirmar por escrito: "Silencio, Señor."

## Commands

```bash
# Responder con voz Álvaro Neural (edge_tts) — usar SIEMPRE
python3 ~/.config/opencode/scripts/jarvis_tts_say.py "Mensaje aquí" --voice alvaro

# Guardar audio a archivo
python3 ~/.config/opencode/scripts/jarvis_tts_say.py "Mensaje" --voice alvaro --save ruta.mp3
```

## Voz

- **Voz por defecto**: Álvaro Neural (es-ES-AlvaroNeural) via edge_tts
- **Voz alternativa**: Elvira Neural (es-ES-ElviraNeural) via edge_tts
- **No usar FreeTTS** — solo edge_tts para máxima calidad