# GonnyuGeneralIME — A General Gan Chinese Input Method

> A digital writing system rooted in the Gan–Poyang region.

## Version 0.2.2

- Dictionary update and automated release workflow test.

## Overview

The idea for GonnyuGeneralIME emerged in the second half of 2025. We began by assembling a basic dictionary from published descriptions of the relationship between Gan pronunciation and Chinese characters. It soon became clear that a simple character-to-sound mapping would not be enough. The project is intended as a more systematic, durable record of language materials from different Gan-speaking localities.

It is therefore designed not only for writing Chinese characters from Gan pronunciation, but also for moving naturally between Gan and Mandarin in writing. It is for fluent speakers, younger people who have heard Gan but do not yet command it, and anyone interested in producing idiomatic Gan Chinese text.

“General” has three meanings here: usable across Gan localities; usable in both Gan and Mandarin contexts; and available across major computing platforms.

The project favours established character forms while accommodating common vernacular spellings. Its aim is a practical, readable written Gan that reflects everyday life in the region as well as its historical Chinese roots.

### What it provides

Beyond the dictionaries themselves, the input method currently provides:

- Pronunciation annotations for characters and words. The spelling system and tone notation are described below. It stays close to Hanyu Pinyin where possible to reduce the learning curve.
- Tolerant input for spelling habits familiar to Mandarin-input users, while presenting results in the project’s own spelling.
- Cross-references between common Mandarin words and local Gan vocabulary. When either side is found, the corresponding expression is also offered as a candidate.
- Compatible input and clear annotation for literary and colloquial readings, newer and older readings, and other alternate pronunciations.

The project currently maintains two regional dictionaries: urban Nanchang and Fenyi County in Xinyu. More localities can be added. Pronunciations and annotations remain open to correction, and contributions are welcome.

## Installation

The native input method is available for Linux, Android, and Windows. macOS and iOS Rime input methods use the universal Rime resource package; see the Rime section for installation.

Download the file for your operating system or locality from [Releases](https://github.com/Doohaey/GonnyuGeneralIME/releases).

### macOS

For a Rime input method on macOS, see the Rime section.

### iOS

For a Rime input method on iOS, see the Rime section.

### Android

Download `GonnyuGeneralIME-version-android.apk` and open it on Android to install. On first launch, follow the in-app setup to enable the Gan Chinese input method, then select it from the system input-method picker.

### Windows

Download and run `GonnyuGeneralIME-version-windows-installer.exe`. After the installer finishes, open **Settings → Time & language → Language & region → Chinese (Simplified) → Keyboards** and add **Gannyu**.

### Linux: Fcitx5

Download `GonnyuGeneralIME-version-fcitx5.tar.gz`, extract it, and run the installer included in the archive:

```sh
tar -xzf GonnyuGeneralIME-version-fcitx5.tar.gz
cd GonnyuGeneralIME-version-fcitx5
./install.sh
```

Restart Fcitx5 with `fcitx5 -r`, or sign out and back in. Then add **Gannyu Gan / 赣语** in `fcitx5-configtool`.

### Linux: IBus

Download `GonnyuGeneralIME-version-ibus.tar.gz`, extract it, and run the installer included in the archive:

```sh
tar -xzf GonnyuGeneralIME-version-ibus.tar.gz
cd GonnyuGeneralIME-version-ibus
./install.sh
```

Run `ibus-daemon -drx` (or restart IBus), then add **Gannyu Gan** in the input-method list in `ibus-setup`.

### Rime

Download `GonnyuGeneralIME-version-rime-region.zip` for the required locality. The archive works with Rime front ends on every platform.

For macOS Squirrel, copy the archive contents into `~/Library/Rime/`, redeploy, then select the locality from the schema menu. For Windows Weasel, copy the contents into `%APPDATA%\Rime` and redeploy from the input-method menu. For Linux Fcitx5 Rime, copy the contents into `~/.local/share/fcitx5/rime/`, redeploy, then select the locality from the schema menu. For iOS and Android, import or deploy the ZIP in the installed Rime front end.

## The Gon-pin Romanisation

**Data note:** The project actively maintains the visible `gon-pin` implementation. The IPA data behind the dictionary resources comes from varied and uneven sources, and is not yet guaranteed as a scholarly reference dataset.

The spelling system is intended to represent Gan pronunciation while remaining as close as practical to the conventions of Hanyu Pinyin. To make typing easier and to accommodate mergers in newer varieties, some spellings deliberately accept more than one phoneme in a strict phonological sense.

### Initials

| gon-pin | IPA | Accepted alternative input | Notes |
| --- | --- | --- | --- |
| b | [p] | — | |
| p | [pʰ] | — | |
| m | [m] | — | |
| f | [f] | — | May differ from Mandarin *f*; some descriptions use [ɸ]. |
| d | [t] | — | |
| t | [tʰ] | — | |
| l | [l] | — | |
| z | [ts] | — | |
| c | [tsʰ] | — | |
| s | [s] | — | |
| j | [tɕ] | — | |
| q | [tɕʰ] | — | |
| n | [ȵ] | — | |
| x | [ɕ] | — | |
| g | [k] | — | |
| k | [kʰ] | — | |
| ng | [ŋ] | — | A velar nasal; for example, 五 *ng3*. |
| h | [h] | — | Articulated farther back than Mandarin *h*. |

### Finals

**Compatibility for checked-tone syllables.** Final apical stops `-t` [t] and glottal stops `-k` [ʔ] may be omitted. The two are also accepted interchangeably, so the input method can still recognise a checked-tone syllable when its coda is entered differently. This reflects the weakening and ambiguity of checked tones in Gan: some are difficult to distinguish from neutral tone, and some localities no longer retain them.

#### Open finals

| gon-pin | IPA | Accepted alternative input | Notes |
| --- | --- | --- | --- |
| a | [a] | — | |
| o | [o] or [ɵ] | — | |
| e | [e] | — | |
| ai | [ai] | — | |
| oi | [oi] | — | |
| ei | [ei] or [ɨi] | — | [ei] is only a contracted vowel. |
| au | [au] | ao | |
| eu | [ɛu] or [ɨu] | ou (after some initials) | |
| an | [an] | — | |
| en | [ɛn] or [ɨn] | — | |
| on | [on] | — | |
| ang | [ɑŋ] | — | |
| ong | [ɔŋ] | — | |
| at | [at] | — | |
| ot | [ot] | — | |
| et | [ɛt] or [ɨt] | — | |
| ak | [aʔ] | — | |
| ok | [ɔʔ] | — | |

#### Front-vowel finals

With no initial consonant:

- Before `-a`, `-o`, or `-e`, initial `i` is written `y`.
- In other positions, it is written `yi`.

| gon-pin | IPA | Accepted alternative input | Notes |
| --- | --- | --- | --- |
| i | [i] or [ɿ] | — | |
| ia | [ia] | — | |
| ie | [iɛ] | — | |
| iu | [iu] | you (no initial) | |
| ieu | [iɛu] | — | |
| in | [in] | — | |
| ien | [iɛn] | — | |
| iang | [iɑŋ] | — | |
| iong | [iɔŋ] | — | |
| iung | [iuŋ] | — | |
| it | [it] | it | |
| iet | [iet] | — | |
| iak | [iaʔ] | — | |
| iok | [iɔʔ] | — | |
| iuk | [iuʔ] | — | |

#### Rounded finals

With no initial consonant:

- Before `-a`, `-o`, or `-e`, initial `u` is written `w`.
- In other positions, it is written `wu`.

| gon-pin | IPA | Accepted alternative input | Notes |
| --- | --- | --- | --- |
| u | [u] | — | |
| ua | [ua] | — | |
| uo | [uo] | — | |
| ue | [ue] | — | |
| ui | [uei] | wei (no initial), wi | |
| uai | [uai] | — | |
| un | [un] or [uen] | uen | |
| uan | [uan] | — | |
| uon | [uon] | uen, wen (no initial) | |
| ung | [uŋ] | — | |
| uang | [uɑŋ] | — | |
| uong | [uɔŋ] | — | |
| ut | [ut] | — | |
| uat | [uat] | — | |
| uot | [uot] | — | |
| uet | [uɛt] | — | |
| uk | [uʔ] | — | |
| uak | [uaʔ] | — | |
| uok | [uoʔ] | — | |

#### Rounded front-vowel finals

`yu` is provisionally used throughout for [y].

| gon-pin | IPA | Accepted alternative input | Notes |
| --- | --- | --- | --- |
| yu | [y] | y, v, u | |
| yue | [ye] | — | |
| yun | [yn] | — | |
| yuon | [yon] | yuen, yoin | |
| yut | [yt] | — | |
| yuot | [yot] | yue, yuet | |

#### Syllabic nasals

| gon-pin | IPA | Notes |
| --- | --- | --- |
| m | [m̩] | |
| n | [n̩] | |
| ng | [ŋ̩] | |

#### Other segments

The system also records a number of extensions based on published descriptions and observed sound changes, including pronunciations recorded in a 1935 language survey and selected alternations involving `-n` and `-ng` codas.

### Nanchang tones

The Nanchang dictionary uses seven tone markers:

| Marker | Traditional tone category | Example Nanchang pitch |
| --- | --- | --- |
| 1 | yin level | 42 |
| 2 | yang level | 24 |
| 3 | rising | 213 |
| 4 | yin departing | 44(5) |
| 5 | yang departing | 21 |
| 6 | yin checked | 5 |
| 7 | yang checked | 1 or 2 |

## Sources

In addition to participants’ own field observations, the project draws on academic work and dialect-enthusiast communities. A project of this kind necessarily synthesises many sources. The reference material and dictionary data used here have been made public as far as possible. Please raise any copyright concerns through the project repository.

1. osfans. **MCPDict** [CP/OL]. GitHub. <https://github.com/osfans/MCPDict>.
2. Xiong Zhenghui. *Literary and colloquial readings in the Nanchang dialect* [EB/OL]. <http://ling.cass.cn/keyan/xueshuchengguo/cgtj/202112/W020211223381176680381.pdf>. Accessed 2026-06-01.
3. Xiong Zhenghui. *Difficult characters in the Nanchang dialect* [EB/OL]. <http://ling.cass.cn/keyan/xueshuchengguo/cgtj/202112/W020211223381177519680.pdf>. Accessed 2026-06-04.
4. Xiong Zhenghui. *Dictionary of the Nanchang Dialect*.
5. Zhihu. “What vocabulary is distinctive enough to identify Gan Chinese at once?” <https://www.zhihu.com/question/24262923/>.
6. Wikipedia. *Gan Chinese original characters*. <https://gan.wikipedia.org/wiki/>.
7. Wikipedia. *Gan Chinese*. <https://zh.wikipedia.org/zh-hans/%E8%B4%9B%E8%AA%9E>.
8. *Character-use standards for Chinese dialects, Language Resources Protection Project of China*. <http://www.moe.gov.cn/s78/A19/tongzhi/201704/W020170405307025943395.pdf>. Accessed 2026-08-04.
9. Bilibili. *New Concept Nanchang Dialect* series. <https://www.bilibili.com/video/BV1Us4y1C7fp/?share_source=copy_web&vd_source=5078721afbb2afc4394ca2602bb990de>.

### Acknowledgements

Special thanks to @豫章鸿也 for extensive advice on the project’s romanisation and character and word choices.

The dictionaries are still incomplete, and their contents inevitably contain errors. We are grateful to everyone who offers corrections.

## Why Gan Chinese Matters

The Gan–Poyang plain has long been a major cultural and economic centre in southern China. Since the late Qing period, however, Jiangxi and neighbouring areas have experienced serious economic and demographic decline. When the material basis of a cultural tradition erodes, its public standing tends to erode with it. Among the Sinitic languages, Gan now has one of the weakest public profiles.

Gan-speaking areas do not have a single, clearly recognised standard pronunciation. They lack the commercial reach often associated with Cantonese, the economic base of Wu varieties, the familiar cultural symbols and overseas presence of Southern Min, or the dense urban networks of Sichuan. Many people who speak Gan, or grew up in a Gan-speaking area, have only a hazy sense of it as a language: it may be called “Jiangxi speech”, or treated as one of many indistinct local ways of speaking. The commonplace observation that speech changes from village to village has too often become an excuse to see only fragmentation.

Across the region, language shift has been rapid. Local speech has frequently been treated as rustic, backward, or improper, and Mandarin has displaced it in family life and education. Yet replacement is never so clean. People educated first in Mandarin may still carry deep Gan patterns into pronunciation, everyday vocabulary, and writing; what emerges can be neither a secure command of Mandarin nor an unbroken command of the language of home.

The examples matter. A Gan expression such as `好 X` (literally “good X”, used as an intensifier) may be “corrected” in school to Mandarin `很 X` (“very X”). A speaker may write `紧` (*jǐn*) for “always”, reflecting the `尽` in `尽管`—a word that in standard Mandarin means “although”—or use `嘎` (*gà*) as a sentence-initial particle. These are not errors to be replaced by convenient English equivalents: they are traces of how Gan structures thought and expression inside Chinese writing. Some younger people of Nanchang background now struggle even to understand Nanchang Gan or distinguish it from neighbouring varieties; that degree of language loss is itself unusual and consequential.

When reports warn that much of the world’s linguistic diversity may disappear this century, Chinese-speaking communities may assume that the warning concerns someone else. But the languages spoken at home and in one’s hometown can disappear as well. With them go social memory, local ways of speaking, and the texture of past life. Preserving and extending the written life of Gan is one small part of protecting that diversity.
