"""Shared Sphinx config for every sbo3l-* Python SDK reference build."""

project = "SBO3L Python SDK reference"
author = "SBO3L"
release = "1.0.1"

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
    "sphinx.ext.viewcode",
    "sphinx.ext.intersphinx",
]

autodoc_typehints = "description"
autodoc_member_order = "bysource"
autodoc_default_options = {
    "members": True,
    "undoc-members": True,
    "show-inheritance": True,
}

napoleon_google_docstring = True
napoleon_numpy_docstring = True

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "pydantic": ("https://docs.pydantic.dev/latest", None),
    "httpx": ("https://www.python-httpx.org", None),
}

html_theme = "furo"
html_title = project
html_show_sourcelink = False
html_show_copyright = False
html_show_sphinx = False

# Mirror the design tokens the Starlight site uses, so the embedded
# pages don't visually clash with the rest of docs.sbo3l.dev.
html_theme_options = {
    "light_css_variables": {
        "color-brand-primary": "#1a8b6c",
        "color-brand-content": "#1a8b6c",
    },
    "dark_css_variables": {
        "color-brand-primary": "#4ad6a7",
        "color-brand-content": "#4ad6a7",
    },
}
