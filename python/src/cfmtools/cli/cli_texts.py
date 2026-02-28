# ============================================================
# HELP SECTIONS
# Everything shown in `cfmtools --help`
# ============================================================

# ---- General ------------------------------------------------

CLI_PROG = "cfmtools"

CLI_USAGE = (
    "cfmtools [GLOBAL OPTIONS] " "STEP COMMAND [ARGS] " "[STEP COMMAND [ARGS] ...]"
)

CLI_DESCRIPTION = (
    "Pipeline-oriented CLI for transforming, sampling, and analyzing CFM models."
)

CLI_VERBOSE_HELP = "Enable verbose logging (show debug output)"


# ---- Section Titles ----------------------------------------

CLI_SECTION_OPTIONS = "options"
CLI_SECTION_STEPS = "pipeline steps"
CLI_SECTION_EXECUTION = "pipeline execution"
CLI_SECTION_TYPICAL_FLOW = "typical flow"
CLI_SECTION_EXAMPLES = "examples"


# ---- Pipeline Steps Section --------------------------------

CLI_STEPS_TITLE = CLI_SECTION_STEPS
CLI_STEPS_DESCRIPTION = None
CLI_STEPS_METAVAR = "STEP"


# ---- Execution Section -------------------------------------

CLI_EXECUTION_TEXT = (
    "Steps are executed strictly left-to-right. "
    "Each step consumes the output of the previous step."
)


# ---- Typical Flow Section ----------------------------------

CLI_TYPICAL_FLOW_TEXT = "load -> (transform)* -> (analyze | sample)* -> export"


# ---- Examples Section --------------------------------------

CLI_EXAMPLES_TEXT = (
    "# Analyze a JSON model and write analysis output:\n"
    "  cfmtools load json --path in.json\n"
    "    analyze semi-structural-sat --output-path out.json --time-limit 20\n"
    "\n"
    "# Convert to bounded form (Big-M) and export JSON:\n"
    "  cfmtools load uvl-fm --path model.uvl\n"
    "    transform big-m\n"
    "    export json --path bounded.json\n"
    "\n"
    "# Pretty-print model to stdout:\n"
    "  cfmtools load json --path in.json\n"
    "    export stdout"
)
