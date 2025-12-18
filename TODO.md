# TODO

## Fix

En terme de norme de codage, est ce que l'on est conforme à la crate iced ? Suit-on les memes règles ?

Splitte le fichier iced-code-editor/src/component.rs pour améliorer la lisibilité.

Ecrit un README.md à destination des futurs utilisateurs du widget en mettant un exemple concret et une capture d'écran du résultat. Le README.md sera sur GitHub, il faut donc qu'il est le look & feel des projets github pro.

Ecrit un DEV.md pour expliquer comment a été développé le widget et comment y contribuer (cargo test, cargo clippy, cargo fmt)

Publier le code dans github: gh repo create
Comment je fais si je veux publier mon widget dans crates.io

[ ] Update function of src/code_editor/component.rs could be split because it's a long function.
[ ] View function of src/code_editor/component.rs could be split by functionnalities (code lines, editor ...)

## Future features

[ ] Undo/Redo with command history
[ ] Search and replace
[ ] Code folding
[ ] Line wrapping
[ ] Minimap (VS Code style)
[ ] Theme selection (light/dark modes)
