# Ресурсы и лицензии

Лицензия MIT из [LICENSE](LICENSE) относится только к коду. У встроенных ресурсов свои лицензии:

| Что | Откуда | Лицензия |
| --- | --- | --- |
| `assets/defaultskin/*` — стандартный скин osu! | [ppy/osu-resources](https://github.com/ppy/osu-resources), © ppy Pty Ltd | [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/) — только некоммерческое использование |
| `assets/icon.svg`, `assets/icon-small.svg` → `icon.png`, `icon.ico` — значок Skinweaver (хит-круг в кольце-косичке; упрощённый вариант для 16–32 px) | этот проект | MIT |
| `assets/fonts/Onest-*.ttf` | The Onest Project Authors, google/fonts | SIL OFL 1.1, `assets/fonts/OFL-Onest.txt` |
| `assets/fonts/CormorantGaramond-SemiBold.ttf` | The Cormorant Project Authors, google/fonts | SIL OFL 1.1, `assets/fonts/OFL-CormorantGaramond.txt` |
| `assets/fonts/JetBrainsMono-Regular.ttf` | The JetBrains Mono Project Authors | SIL OFL 1.1, `assets/fonts/OFL.txt` |
| `assets/icons/*.svg` | [Lucide](https://lucide.dev) | ISC, `assets/icons/LICENSE.txt` |
| Зависимости Rust | crates.io | см. [THIRD_PARTY_LICENSES.txt](THIRD_PARTY_LICENSES.txt) |

Изменения:
- стандартный скин встроен в exe без изменений; демо-библиотека (`--demo`) перекрашивает его копии во временной папке;
- из вариативных Onest и Cormorant Garamond вырезаны статичные начертания, оставлено подмножество latin/cyrillic;
- у иконок Lucide штрих 1.75 и белый цвет, чтобы их можно было тонировать.

Из-за CC BY-NC 4.0 сборку со встроенным стандартным скином нельзя продавать или использовать
в коммерческом продукте. Код без этих файлов — под MIT.

Предпросмотр показывает только локальные файлы скинов: они никуда не отправляются и в сборку не попадают.
`docs/screenshots/` — снимки приложения с демо-библиотекой.
