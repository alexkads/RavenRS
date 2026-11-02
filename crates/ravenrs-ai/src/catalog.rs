//! The models this crate knows, each pinned to a published version.

use crate::{LocalModel, ModelFamily, ModelFile};

/// The models that can be offered, smallest first. Only their files are listed: a
/// model comes to the machine only when someone asks for it.
///
/// Memory was measured at its peak on an Apple M2 Pro for Qwen3 1.7B, Qwen 2.5 0.5B and
/// Llama 3.2 1B; the others are scaled from their family, and
/// [`ModelManager`](crate::ModelManager)'s speed test lets each machine check them.
/// Gemma 3 270M is not here on purpose: it ran, and understood none of the requests.
pub const CATALOG: &[LocalModel] = &[
    LocalModel {
        id: "qwen3-0.6b-q4",
        name: "Qwen3 0.6B",
        license: "Apache 2.0",
        family: ModelFamily::Qwen3,
        memory: 1_541_597_464,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/resolve/50968a4468ef4233ed78cd7c3de230dd1d61a56b/Qwen3-0.6B-Q4_K_M.gguf",
                bytes: 396_705_472,
                sha256: "ac2d97712095a558e31573f62f466a3f9d93990898b0ec79d7c974c1780d524a",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/Qwen/Qwen3-0.6B/resolve/c1899de289a04d12100db370d81485cdf75e47ca/tokenizer.json",
                bytes: 11_422_654,
                sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
            },
        ],
    },
    LocalModel {
        id: "qwen2.5-0.5b-q4",
        name: "Qwen 2.5 0.5B",
        license: "Apache 2.0",
        family: ModelFamily::Qwen2,
        memory: 1_388_807_896,
        understood: Some(34),
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/9217f5db79a29953eb74d5343926648285ec7e67/qwen2.5-0.5b-instruct-q4_k_m.gguf",
                bytes: 491_400_032,
                sha256: "74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct/resolve/7ae557604adf67be50417f59c2c2f167def9a775/tokenizer.json",
                bytes: 7_031_645,
                sha256: "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539",
            },
        ],
    },
    LocalModel {
        id: "llama3.2-1b-q4",
        name: "Llama 3.2 1B",
        license: "Llama 3.2",
        family: ModelFamily::Llama3,
        memory: 3_399_372_136,
        understood: Some(47),
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/067b946cf014b7c697f3654f621d577a3e3afd1c/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
                bytes: 807_694_464,
                sha256: "6f85a640a97cf2bf5b8e764087b1e83da0fdb51d7c9fab7d0fece9385611df83",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/unsloth/Llama-3.2-1B-Instruct/resolve/5a8abab4a5d6f164389b1079fb721cfab8d7126c/tokenizer.json",
                bytes: 17_209_920,
                sha256: "6b9e4e7fb171f92fd137b777cc2714bf87d11576700a1dcd7a399e7bbe39537b",
            },
        ],
    },
    LocalModel {
        id: "gemma3-1b-q4",
        name: "Gemma 3 1B",
        license: "Gemma",
        family: ModelFamily::Gemma3,
        memory: 6_300_000_000,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/bartowski/google_gemma-3-1b-it-GGUF/resolve/116f76234503685a98f572982177b11d44ec8ff1/google_gemma-3-1b-it-Q4_K_M.gguf",
                bytes: 806_058_496,
                sha256: "12bf0fff8815d5f73a3c9b586bd8fee8e7b248c935de70dec367679873d0f29d",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/unsloth/gemma-3-1b-it/resolve/5b11413a10db4e486ef16a20101fd028f8f2499c/tokenizer.json",
                bytes: 33_384_568,
                sha256: "4667f2089529e8e7657cfb6d1c19910ae71ff5f28aa7ab2ff2763330affad795",
            },
        ],
    },
    LocalModel {
        id: "qwen3-1.7b-q4",
        name: "Qwen3 1.7B",
        license: "Apache 2.0",
        family: ModelFamily::Qwen3,
        memory: 4_303_376_896,
        understood: Some(83),
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/unsloth/Qwen3-1.7B-GGUF/resolve/d7f544eead698dbd1f15126ef60b45a1e1933222/Qwen3-1.7B-Q4_K_M.gguf",
                bytes: 1_107_409_472,
                sha256: "b139949c5bd74937ad8ed8c8cf3d9ffb1e99c866c823204dc42c0d91fa181897",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/Qwen/Qwen3-1.7B/resolve/70d244cc86ccca08cf5af4e1e306ecf908b1ad5e/tokenizer.json",
                bytes: 11_422_654,
                sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
            },
        ],
    },
    LocalModel {
        id: "qwen2.5-1.5b-q4",
        name: "Qwen 2.5 1.5B",
        license: "Apache 2.0",
        family: ModelFamily::Qwen2,
        memory: 3_200_000_000,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/91cad51170dc346986eccefdc2dd33a9da36ead9/qwen2.5-1.5b-instruct-q4_k_m.gguf",
                bytes: 1_117_320_736,
                sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct/resolve/989aa7980e4cf806f80c7fef2b1adb7bc71aa306/tokenizer.json",
                bytes: 7_031_645,
                sha256: "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539",
            },
        ],
    },
    LocalModel {
        id: "llama3.2-3b-q4",
        name: "Llama 3.2 3B",
        license: "Llama 3.2",
        family: ModelFamily::Llama3,
        memory: 8_500_000_000,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/5ab33fa94d1d04e903623ae72c95d1696f09f9e8/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
                bytes: 2_019_377_696,
                sha256: "6c1a2b41161032677be168d354123594c0e6e67d2b9227c84f296ad037c728ff",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/unsloth/Llama-3.2-3B-Instruct/resolve/006f5dcd1393c3add266de40994ba96225e9689d/tokenizer.json",
                bytes: 17_209_920,
                sha256: "6b9e4e7fb171f92fd137b777cc2714bf87d11576700a1dcd7a399e7bbe39537b",
            },
        ],
    },
    LocalModel {
        id: "phi3-mini-q4",
        name: "Phi-3 Mini",
        license: "MIT",
        family: ModelFamily::Phi3,
        memory: 9_300_095_945,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/microsoft/Phi-3-mini-4k-instruct-gguf/resolve/a64113399c2f6b8ad3e11c394733a2ddadaa7f33/Phi-3-mini-4k-instruct-q4.gguf",
                bytes: 2_393_231_072,
                sha256: "8a83c7fb9049a9b2e92266fa7ad04933bb53aa1e85136b7b30f1b8000ff2edef",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/microsoft/Phi-3-mini-4k-instruct/resolve/f39ac1d28e925b323eae81227eaba4464caced4e/tokenizer.json",
                bytes: 1_937_869,
                sha256: "072ab882d6c7192a42f78790945d16c064691321a73251a4b18f6a380f0fbe39",
            },
        ],
    },
    LocalModel {
        id: "qwen3-4b-q4",
        name: "Qwen3 4B",
        license: "Apache 2.0",
        family: ModelFamily::Qwen3,
        memory: 9_704_435_178,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/unsloth/Qwen3-4B-GGUF/resolve/22c9fc8a8c7700b76a1789366280a6a5a1ad1120/Qwen3-4B-Q4_K_M.gguf",
                bytes: 2_497_281_312,
                sha256: "f6f851777709861056efcdad3af01da38b31223a3ba26e61a4f8bf3a2195813a",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/Qwen/Qwen3-4B/resolve/1cfa9a7208912126459214e8b04321603b3df60c/tokenizer.json",
                bytes: 11_422_654,
                sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
            },
        ],
    },
    LocalModel {
        id: "gemma3-4b-q4",
        name: "Gemma 3 4B",
        license: "Gemma",
        family: ModelFamily::Gemma3,
        memory: 12_900_000_000,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/bartowski/google_gemma-3-4b-it-GGUF/resolve/71506238f970075ca85125cd749c28b1b0eee84e/google_gemma-3-4b-it-Q4_K_M.gguf",
                bytes: 2_489_758_112,
                sha256: "4996030242583a40aa151ff93f49ed787ac8c25e4120c3ae4588b2e2a7d1ae94",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/unsloth/gemma-3-4b-it/resolve/bf46152c47f5dd20b896357cb51abc4c03b8ee8c/tokenizer.json",
                bytes: 33_384_568,
                sha256: "4667f2089529e8e7657cfb6d1c19910ae71ff5f28aa7ab2ff2763330affad795",
            },
        ],
    },
    LocalModel {
        id: "mistral-7b-q4",
        name: "Mistral 7B",
        license: "Apache 2.0",
        family: ModelFamily::Mistral,
        memory: 16_975_756_223,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/TheBloke/Mistral-7B-Instruct-v0.2-GGUF/resolve/3a6fbf4a41a1d52e415a4958cde6856d34b2db93/mistral-7b-instruct-v0.2.Q4_K_M.gguf",
                bytes: 4_368_439_584,
                sha256: "3e0039fd0273fcbebb49228943b17831aadd55cbcbf56f0af00499be2040ccf9",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/mistralai/Mistral-7B-Instruct-v0.2/resolve/63a8b081895390a26e140280378bc85ec8bce07a/tokenizer.json",
                bytes: 1_795_188,
                sha256: "11c08db21487c885d8c792180f0be237f6a261b89a46f128a6a80a3aa4bd1720",
            },
        ],
    },
    LocalModel {
        id: "qwen3-8b-q4",
        name: "Qwen3 8B",
        license: "Apache 2.0",
        family: ModelFamily::Qwen3,
        memory: 19_537_970_613,
        understood: None,
        files: &[
            ModelFile {
                name: "model.gguf",
                url: "https://huggingface.co/unsloth/Qwen3-8B-GGUF/resolve/a6adef130ffb23ddaf1a62fec9dced968c9bc482/Qwen3-8B-Q4_K_M.gguf",
                bytes: 5_027_784_512,
                sha256: "120307ba529eb2439d6c430d94104dabd578497bc7bfe7e322b5d9933b449bd4",
            },
            ModelFile {
                name: "tokenizer.json",
                url: "https://huggingface.co/Qwen/Qwen3-8B/resolve/b968826d9c46dd6066d109eabc6255188de91218/tokenizer.json",
                bytes: 11_422_654,
                sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
            },
        ],
    },
];

/// A model of [`CATALOG`] by its id.
pub fn model_by_id(id: &str) -> Option<LocalModel> {
    CATALOG.iter().find(|model| model.id == id).copied()
}
