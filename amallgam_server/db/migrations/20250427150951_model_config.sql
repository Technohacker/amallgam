/**
* Table for Model Details
*/
CREATE TABLE model_config(
    -- Model ID
    id VARCHAR(64) PRIMARY KEY,

    -- Model File name
    file_name VARCHAR(1024) NOT NULL,

    -- Prompt Template
    prompt_template VARCHAR(1024) NOT NULL
);
