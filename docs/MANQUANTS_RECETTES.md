# Recettes de MineColonies — 154 copiées, 4 exclues

Source : `minecolonies-src/src/datagen/generated/minecolonies/data/minecolonies/recipes/`
(158 fichiers). Copiées **verbatim** — vérifié fichier par fichier : 154/154 identiques à la source,
0 fichier invalide.

Répartition : 88 `crafting_shaped`, 46 `crafting_shapeless`, 7 `smelting`, 6 `smoking`,
6 `campfire_cooking`, 1 `blasting`.

## Les 4 exclues, et pourquoi

Ces quatre fichiers déclarent un **type de recette propre à MineColonies**, donc un sérialiseur
enregistré par du code — il n'existe pas dans Horizon aujourd'hui :

| Fichier | Type déclaré |
|---|---|
| `compostables.json` | `minecolonies:composting` |
| `compostables_poor.json` | `minecolonies:composting` |
| `compostables_rich.json` | `minecolonies:composting` |
| `soy_milk.json` | `minecolonies:zero_waste` |

Les copier telles quelles ferait refuser le contenu au chargement : un `type` inconnu n'a pas de
désérialiseur. Elles reviendront quand ces deux types seront enregistrés côté code du port
(ou déclarés en `[[native.mixin]]` sur le registre des recettes).

## Ce qui a été écarté

Une première passe avait **restructuré** les fichiers au lieu de les copier — les ingrédients
`{"item": "x"}` devenaient des chaînes nues et le `count` du résultat disparaissait. 40 fichiers
sur 40 échantillonnés différaient de la source. Tout a été refait en copie verbatim.

**Règle** : un fichier de données se copie, il ne se réécrit pas. Un portage de contenu ne
« normalise » rien — sinon on ne porte plus la source, on en écrit une variante non vérifiée.
