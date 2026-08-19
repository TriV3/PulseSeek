# Metering traceability

**Status:** frozen planning contract for PR-092
**Authority:** `spec/implementation-plan.md` and four metering specifications

The matrix is exhaustive for all 152 requirement IDs defined by the product and metering specifications.

Every metering requirement has one planned owner PR. PR-092 freezes this
mapping; later PRs add implementation files and test evidence without changing
ownership silently. `FR-VS-001..010` remain owned by their existing
visualization PRs and are included in the product-level contract.

## Evidence contract

Each row below uses this compact form:

`requirement | owner | implementation | automated evidence | manual evidence | reference | limitation`

`planned` means evidence is required from the owning implementation PR.
`traceability` means PR-092 validates the mapping but does not implement
behavior. `not applicable` means no manual check is required for that row.

## Requirement matrix

| Requirement | Owner PR | Implementation | Automated evidence | Manual evidence | Reference | Limitation |
|---|---:|---|---|---|---|---|
| FR-BE-001 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-002 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-003 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-004 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-005 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-006 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-007 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-008 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-009 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-BE-010 | PR-107 | band product/profile service | band boundary tests | band profile check | functional §6; DSP §8 | custom profiles downstream |
| FR-CA-001 | PR-117..119 | analysis cache/index | cache migration tests | cache privacy check | functional §14; architecture §8–9 | no manager item |
| FR-CA-002 | PR-117..119 | analysis cache/index | cache migration tests | cache privacy check | functional §14; architecture §8–9 | no manager item |
| FR-CA-003 | PR-117..119 | analysis cache/index | cache migration tests | cache privacy check | functional §14; architecture §8–9 | no manager item |
| FR-CA-004 | PR-117..119 | analysis cache/index | cache migration tests | cache privacy check | functional §14; architecture §8–9 | no manager item |
| FR-CA-005 | PR-117..119 | analysis cache/index | cache migration tests | cache privacy check | functional §14; architecture §8–9 | no manager item |
| FR-CA-006 | PR-117..119 | analysis cache/index | cache migration tests | cache privacy check | functional §14; architecture §8–9 | no manager item |
| FR-CA-007 | PR-117..119 | analysis cache/index | cache migration tests | cache privacy check | functional §14; architecture §8–9 | no manager item |
| FR-CF-001 | PR-116 | profile/degradation service | migration/degradation tests | quality profile check | functional §12; architecture §6 | budget evidence downstream |
| FR-CF-002 | PR-116 | profile/degradation service | migration/degradation tests | quality profile check | functional §12; architecture §6 | budget evidence downstream |
| FR-CF-003 | PR-116 | profile/degradation service | migration/degradation tests | quality profile check | functional §12; architecture §6 | budget evidence downstream |
| FR-CF-004 | PR-116 | profile/degradation service | migration/degradation tests | quality profile check | functional §12; architecture §6 | budget evidence downstream |
| FR-CF-005 | PR-116 | profile/degradation service | migration/degradation tests | quality profile check | functional §12; architecture §6 | budget evidence downstream |
| FR-CF-006 | PR-116 | profile/degradation service | migration/degradation tests | quality profile check | functional §12; architecture §6 | budget evidence downstream |
| FR-CF-007 | PR-116 | profile/degradation service | migration/degradation tests | quality profile check | functional §12; architecture §6 | budget evidence downstream |
| FR-CF-008 | PR-116 | profile/degradation service | migration/degradation tests | quality profile check | functional §12; architecture §6 | budget evidence downstream |
| FR-CW-001 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-CW-002 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-CW-003 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-CW-004 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-CW-005 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-CW-006 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-CW-007 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-CW-008 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-CW-009 | PR-108 | colored waveform product/cache | coverage/cache tests | coverage rendering check | functional §7; validation §4.5 | display-only color transforms |
| FR-DS-001 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-002 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-003 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-004 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-005 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-006 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-007 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-008 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-009 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-DS-010 | PR-115 | decision-support products | experimental module tests | opt-in UX check | functional §11; validation §4.8 | no automatic verdict |
| FR-EX-001 | PR-133..135 | external source adapters/controls | permission lifecycle tests | privacy and stop check | functional §13; architecture §2/§10 | platform adapters separate |
| FR-EX-002 | PR-133..135 | external source adapters/controls | permission lifecycle tests | privacy and stop check | functional §13; architecture §2/§10 | platform adapters separate |
| FR-EX-003 | PR-133..135 | external source adapters/controls | permission lifecycle tests | privacy and stop check | functional §13; architecture §2/§10 | platform adapters separate |
| FR-EX-004 | PR-133..135 | external source adapters/controls | permission lifecycle tests | privacy and stop check | functional §13; architecture §2/§10 | platform adapters separate |
| FR-EX-005 | PR-133..135 | external source adapters/controls | permission lifecycle tests | privacy and stop check | functional §13; architecture §2/§10 | platform adapters separate |
| FR-EX-006 | PR-133..135 | external source adapters/controls | permission lifecycle tests | privacy and stop check | functional §13; architecture §2/§10 | platform adapters separate |
| FR-LD-001 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-002 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-003 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-004 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-005 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-006 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-007 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-008 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-009 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-010 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-LD-011 | PR-110..112 | loudness and true-peak products | calibration/lifecycle tests | reference meter comparison | functional §9; DSP §10–12; validation §3 | continuous gaps incomplete |
| FR-MS-001 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-002 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-003 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-004 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-005 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-006 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-007 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-008 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-009 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-010 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-011 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-012 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-013 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MS-014 | PR-094, PR-098 | analysis source/session contract | source lifecycle tests | source lifecycle check | functional §4; architecture §2–3 | adapter-specific evidence downstream |
| FR-MW-001 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-002 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-003 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-004 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-005 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-006 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-007 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-008 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-009 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-010 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-011 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-012 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-013 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-014 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-015 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-016 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-017 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-018 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-019 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-MW-020 | PR-103..105 | workspace and tile service | workspace behavior tests | keyboard workspace check | metering-functional §3; validation §4.1 | layout migration and experimental policy |
| FR-SG-001 | PR-109 | spectrogram product | timestamp/history tests | bounded history check | functional §8; validation §4.5 | latest-only display |
| FR-SG-002 | PR-109 | spectrogram product | timestamp/history tests | bounded history check | functional §8; validation §4.5 | latest-only display |
| FR-SG-003 | PR-109 | spectrogram product | timestamp/history tests | bounded history check | functional §8; validation §4.5 | latest-only display |
| FR-SG-004 | PR-109 | spectrogram product | timestamp/history tests | bounded history check | functional §8; validation §4.5 | latest-only display |
| FR-SG-005 | PR-109 | spectrogram product | timestamp/history tests | bounded history check | functional §8; validation §4.5 | latest-only display |
| FR-SG-006 | PR-109 | spectrogram product | timestamp/history tests | bounded history check | functional §8; validation §4.5 | latest-only display |
| FR-SG-007 | PR-109 | spectrogram product | timestamp/history tests | bounded history check | functional §8; validation §4.5 | latest-only display |
| FR-SG-008 | PR-109 | spectrogram product | timestamp/history tests | bounded history check | functional §8; validation §4.5 | latest-only display |
| FR-SP-001 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-002 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-003 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-004 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-005 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-006 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-007 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-008 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-009 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-010 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-011 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-012 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-013 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-SP-014 | PR-100..101, PR-106 | FFT bank and spectrum product | FFT/product tests | calibrated spectrum check | functional §5; DSP §1–3 | raw bins remain Rust |
| FR-ST-001 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-ST-002 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-ST-003 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-ST-004 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-ST-005 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-ST-006 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-ST-007 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-ST-008 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-ST-009 | PR-113..114 | stereo product | channel transform tests | stereo fixture check | functional §10; DSP §7 | normalization documented downstream |
| FR-UI-001 | PR-120..122 | meter tile/accessibility UI | RTL behavior tests | keyboard/high-contrast check | functional §15; validation §4.1 | React display-only |
| FR-UI-002 | PR-120..122 | meter tile/accessibility UI | RTL behavior tests | keyboard/high-contrast check | functional §15; validation §4.1 | React display-only |
| FR-UI-003 | PR-120..122 | meter tile/accessibility UI | RTL behavior tests | keyboard/high-contrast check | functional §15; validation §4.1 | React display-only |
| FR-UI-004 | PR-120..122 | meter tile/accessibility UI | RTL behavior tests | keyboard/high-contrast check | functional §15; validation §4.1 | React display-only |
| FR-UI-005 | PR-120..122 | meter tile/accessibility UI | RTL behavior tests | keyboard/high-contrast check | functional §15; validation §4.1 | React display-only |
| FR-UI-006 | PR-120..122 | meter tile/accessibility UI | RTL behavior tests | keyboard/high-contrast check | functional §15; validation §4.1 | React display-only |
| FR-VS-001 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-002 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-003 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-004 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-005 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-006 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-007 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-008 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-009 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| FR-VS-010 | PR-080..087 | existing visualization modules | existing visualization tests | playback visualization check | functional §4.4; FR-VS contract | compatibility evidence |
| NFR-MT-001 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-002 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-003 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-004 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-005 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-006 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-007 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-008 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-009 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
| NFR-MT-010 | PR-092, downstream owner | cross-cutting metering boundary | cross-cutting validation | release evidence | functional §7.6; validation §8 | traceability-only at PR-092 |
