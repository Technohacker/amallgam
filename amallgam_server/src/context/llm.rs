use std::sync::Arc;

use anyhow::Result;
use llama_cpp::{LlamaModel, LlamaParams, SessionParams};
use minijinja::Environment;

use crate::objects::users::User;

use super::{AmallgamContext, model_config::LoadedModel};

impl AmallgamContext {
    pub(crate) async fn run_llm_inference(
        &self,
        bot_user: &User,
        // sender_name: impl AsRef<str>,
        message: impl AsRef<str>,
    ) -> Result<String> {
        let User::Local {
            user_id,
            model_id,
            system_prompt,
            ..
        } = bot_user
        else {
            return Err(anyhow::format_err!(
                "Attempted to use a remote user as a bot"
            ));
        };
        let loaded_model = self
            .llama_models
            .try_get_with_by_ref(model_id, async {
                let model_config = self
                    .get_model_config_by_id(model_id)
                    .await?
                    .ok_or_else(|| anyhow::format_err!("Model not found"))?;

                let model_path = self.config.models_folder.join(model_config.file_name);
                let mut env = Environment::new();
                env.add_template_owned("prompt_template", model_config.prompt_template)?;

                let loaded_model = LoadedModel {
                    model: LlamaModel::load_from_file_async(
                        &model_path,
                        LlamaParams {
                            n_gpu_layers: 0,
                            ..Default::default()
                        },
                    )
                    .await?,
                    template_env: Arc::new(env),
                };

                anyhow::Ok(loaded_model)
            })
            .await
            .map_err(|x| anyhow::format_err!("Error loading model: {x}"))?;

        let prompt_template = loaded_model
            .template_env
            .get_template("prompt_template")
            .expect("Prompt template missing from cache?");

        let bos_token = loaded_model.model.decode_tokens([loaded_model.model.bos()]);
        let eos_token = loaded_model.model.decode_tokens([loaded_model.model.eos()]);

        let mut session = self
            .llama_sessions
            .try_get_with_by_ref(user_id, async {
                let mut session = loaded_model.model.create_session(SessionParams {
                    n_threads: self.config.num_cores_per_session,
                    ..Default::default()
                })?;

                let prompt = prompt_template.render(Self::make_jinja_context(
                    system_prompt,
                    &bos_token,
                    &eos_token,
                    None,
                ))?;

                session
                    .set_context_to_tokens_async(
                        session.model().tokenize_bytes(prompt, false, true)?,
                    )
                    .await?;

                anyhow::Ok(session)
            })
            .await
            .map_err(|err| anyhow::format_err!("Error occured when preparing LLM Session: {err}"))?
            .deep_copy()?;

        let prompt = prompt_template.render(Self::make_jinja_context(
            system_prompt,
            &bos_token,
            &eos_token,
            Some(message.as_ref()),
        ))?;

        session
            .set_context_to_tokens_async(session.model().tokenize_bytes(prompt, false, true)?)
            .await?;

        Ok(session.start_completing()?.into_string_async().await)
    }

    fn make_jinja_context(
        system_prompt: &str,
        bos_token: &str,
        eos_token: &str,
        user_message: Option<&str>,
    ) -> minijinja::Value {
        let mut messages = vec![minijinja::context! {
            role => "system",
            content => system_prompt,
        }];

        if let Some(user_message) = user_message {
            messages.push(minijinja::context! {
                role => "system",
                content => user_message,
            });
        }

        minijinja::context! {
            messages,
            add_generation_prompt => true,
            bos_token,
            eos_token,
        }
    }
}
