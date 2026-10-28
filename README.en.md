# GonnyuGeneralIME — A General Gon(Gan) Chinese Input Method | GonPin

[简体中文](README.md) | **English**

> A digital writing system rooted in the Gon(Gan)–Poyang region.

Currently supports: **Lancong (Nanchang), Fenni (Fenyi), Fungcen (Fengcheng), Tiqien (Lichuan), Sinyi (Xinyu City), Songau (Shanggao), Seusong (Susong), Jingon (Ji'an), Yikyan-Henfeng (Yiyang and Hengfeng), Jisuibaedu (Jishuibadou), Xyucuênn (Suichuan), Singzï (Xingzi), and Goungon (Goungon)**. Contributions are welcome for additional localities and additions to existing regional dictionaries.

[![Contribute · Contribution guide (Chinese)](https://img.shields.io/badge/Contribute-Guide%20%28Chinese%29-006d77?style=for-the-badge&logo=github&logoColor=white)](CONTRIBUTION.md)

**An easy-to-install Gon(Gan) input method for everyday use. Users familiar with Pinyin can get started with compatible support for Gon(Gan) romanisation and Mandarin Pinyin. Even users who do not speak Gon(Gan) can explore it, and Gon(Gan) expressions can also be used to write extended passages in Mandarin, including text like this document.**

**Now available on the iOS App Store as 赣语输入法. Native installation is available on macOS, Android, Windows, and Linux, alongside Rime resource packages.**

[![Rime Lancong (Nanchang)](https://img.shields.io/badge/Rime-Lancong%20%28Nanchang%29-c62828?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Doohaey/GonnyuGeneralIME-Rime-Lancong)
[![Rime Fenni (Fenyi)](https://img.shields.io/badge/Rime-Fenni%20%28Fenyi%29-c05621?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Doohaey/GonnyuGeneralIME-Rime-Fenni)
[![Rime Fungcen](https://img.shields.io/badge/Rime-Fungcen-facc15?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Doohaey/GonnyuGeneralIME-Rime-Fungcen)
[![Rime Yikyan-Henfeng](https://img.shields.io/badge/Rime-Yikyan--Henfeng-1f883d?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Doohaey/GonnyuGeneralIME-Rime-Yikyan-Henfeng)
[![Rime Sinyi (Xinyu)](https://img.shields.io/badge/Rime-Sinyi%20%28Xinyu%29-0969da?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Doohaey/GonnyuGeneralIME-Rime-Sinyi)
[![Rime Goungon (Gao'an)](https://img.shields.io/badge/Rime-Goungon%20%28Gao%27an%29-8250df?style=for-the-badge&logo=github&logoColor=white)](https://github.com/Doohaey/GonnyuGeneralIME-Rime-Goungon)

Rime schema repositories are available for Lancong (Nanchang), Fenni (Fenyi), Fungcen, Yikyan-Henfeng, Sinyi (Xinyu), and Goungon (Gao'an). Other installation options are available in the [Installation](#installation) section below.

## Release 1.2.5

- data(dictionary): Update the Lancong dictionary and clean up the Fenni dictionary.
- feat(input): Add alternating Chinese double quotes on desktop.
- fix(input): Use separate left and right Chinese quote keys on the mobile 123 page.

## Contents

- [GonnyuGeneralIME — A General Gon(Gan) Chinese Input Method](#gonnyugeneralime--a-general-gongan-chinese-input-method)
  - [Release 1.2.5](#release-125)
  - [Contents](#contents)
  - [Overview](#overview)
    - [What it provides](#what-it-provides)
  - [See Gan in action](#see-gan-in-action)
    - [No learning needed, type by instinct](#no-learning-needed-type-by-instinct)
    - [Broad vocabulary, local life in full, rare characters no longer a barrier](#broad-vocabulary-local-life-in-full-rare-characters-no-longer-a-barrier)
    - [Gan and Mandarin, side by side](#gan-and-mandarin-side-by-side)
    - [Mandarin Pinyin, straight to Gan](#mandarin-pinyin-straight-to-gan)
    - [Literary or colloquial — clear at a glance](#literary-or-colloquial--clear-at-a-glance)
    - [Every Gan reading welcome, old and new](#every-gan-reading-welcome-old-and-new)
  - [Installation](#installation)
    - [macOS](#macos)
    - [iOS](#ios)
    - [Android](#android)
    - [Windows](#windows)
    - [Linux Fcitx5](#linux-fcitx5)
    - [Rime](#rime)
    - [Input settings](#input-settings)
  - [The General Gon-pin Romanisation](#the-general-gon-pin-romanisation)
    - [Initials](#initials)
    - [Finals](#finals)
      - [Open finals](#open-finals)
      - [Front-vowel finals](#front-vowel-finals)
      - [Rounded finals](#rounded-finals)
      - [Rounded front-vowel finals](#rounded-front-vowel-finals)
      - [Syllabic laterals and nasals](#syllabic-laterals-and-nasals)
      - [Other segments](#other-segments)
    - [Tones](#tones)
      - [Lancong (Nanchang)](#lancong-nanchang)
      - [Fungcen (Fengcheng)](#fungcen-fengcheng)
      - [Tiqien (Lichuan)](#tiqien-lichuan)
      - [Sinyi (Xinyu City)](#sinyi-xinyu-city)
      - [Songau (Shanggao)](#songau-shanggao)
      - [Seusong (Susong)](#seusong-susong)
      - [Jingon (Jian City)](#jingon-jian-city)
      - [Yikyan-Henfeng (Yiyang-Hengfeng)](#yikyan-henfeng-yiyang-hengfeng)
      - [Jisuibaedu (Jishuibadou)](#jisuibaedu-jishuibadou)
      - [Xyucuênn (Suichuan)](#xyucuênn-suichuan)
      - [Singzï (Xingzi)](#singzï-xingzi)
      - [Goungon (Goungon)](#goungon-goungon)
  - [References](#references)
    - [Literature](#literature)
    - [Reference implementations](#reference-implementations)
    - [Dependency declarations](#dependency-declarations)
    - [Licensing and rights reservation](#licensing-and-rights-reservation)
    - [Acknowledgements](#acknowledgements)
  - [Origins: the loss of Gon(Gan)](#origins-the-loss-of-gongan)
  - [Contributors and contact](#contributors-and-contact)

## Overview

In the second half of 2025, we began thinking about creating a general Gon(Gan) input method, after observing input methods for other Sinitic languages. We initially assembled a basic dictionary by drawing on several sources to organise the correspondences between Gon(Gan) pronunciation and Chinese characters. We soon felt that simple sound-to-character mappings were insufficient for our needs. We wanted a more systematic way to organise and preserve language materials from different Gon(Gan)-speaking localities. The resulting project supports typing characters by their Gon(Gan) pronunciation, writing Mandarin through Gon(Gan) expressions, and producing idiomatic Gon(Gan) text even for younger people who have heard the language but are not proficient in it, or enthusiasts interested in Gon(Gan).

“General” has three meanings: use across Gon(Gan) localities, use across Gon(Gan) and Mandarin, and technical support across platforms.

Our project favours etymologically supported and standard character forms while accommodating common vernacular spellings, helping users write local Chinese that combines standard usage with familiar forms.

### What it provides

Beyond the dictionaries themselves, the input method currently provides:

- Pronunciation annotations for characters and words. The spelling system and tone notation are described below. It stays close to Hanyu Pinyin where possible to reduce the learning curve.
- Tolerant input for spelling habits familiar to Mandarin-input users, while presenting results in the project’s own spelling.
- Cross-references between common Mandarin words and local Gon(Gan) vocabulary. When either side is found, the corresponding expression is also offered as a candidate.
- Compatible input and clear annotation for literary and colloquial readings, newer and older readings, and other alternate pronunciations.

The project currently maintains 13 regional dictionaries: urban Lancong (Nanchang), Fenni (Fenyi County in Xinyu), Fungcen, Tiqien (Lichuan), Sinyi (Xinyu City), Songau (Shanggao), Seusong (Susong), Jingon (Jian City), Yikyan-Henfeng (Yiyang-Hengfeng), Jisuibaedu (Jishuibadou), Xyucuênn (Suichuan), Singzï (Xingzi), and Goungon (Goungon). We hope to expand substantially to other localities as well. Contributions to add and correct dictionary entries are welcome.

## See Gan in action

<sub>The examples below use Nanchang Gan.</sub>

### No learning needed, type by instinct

Know Mandarin Pinyin and start typing right away. Enter a familiar Pinyin spelling, and the input method finds the corresponding Gan reading without requiring a separate input scheme.

![No learning needed, type by instinct](resources/images/selection/yue.png)

### Broad vocabulary, local life in full, rare characters no longer a barrier

Over 20,000 Chinese characters, including extensive coverage of Unicode Extension B with theoretical readings derived from rhyme dictionaries; over 100,000 words; abundant idiomatic local expressions; and distinctive pronunciations for place names. The dictionary goes far beyond a bare list of character readings.

![Broad coverage of local expressions](resources/images/selection/yongxyuot.png)
![Distinctive pronunciation for the place name Youkou](resources/images/selection/xiukieu.png)

### Gan and Mandarin, side by side

Type in Gan and see related Mandarin words at the same time. Recognise, confirm, and choose the word you want in one glance.

![Gan and Mandarin candidates together](resources/images/selection/goxiet.png)

### Mandarin Pinyin, straight to Gan

Have the Mandarin word in mind first? Type its familiar Pinyin and find the Gan expression right away.

![Type Gan text with Mandarin Pinyin](resources/images/selection/yitiandaowan.png)

### Literary or colloquial — clear at a glance

Gan everyday speech and written expression can sound different. The input method lays both readings out clearly, so your writing always fits the moment.

![Literary and colloquial reading example: miangnit](resources/images/selection/miangnit.png)

![Literary and colloquial reading example: minceu](resources/images/selection/minceu.png)

### Every Gan reading welcome, old and new

Alternate Gan pronunciations, including older and newer patterns, are all ready for input and lookup. Type the Gan you know, your way.

![Compatibility with alternate, older, and newer pronunciations](resources/images/selection/xiuji.png)


## Installation

The native input method is available for Linux, Android, Windows, and macOS. Installation of the universal Rime resource packages is described in the Rime section.

Packages for each operating system or locality are available from [Releases](https://github.com/Doohaey/GonnyuGeneralIME/releases).

### macOS

Download `GonnyuGeneralIME-version-macos.pkg` from [Releases](https://github.com/Doohaey/GonnyuGeneralIME/releases), open it, and complete the installation.

If it does not appear automatically, open **System Settings → Keyboard → Text Input → Edit…**, click **+**, search for and add **Gonnyu**, then select it from the input menu. Upgrades preserve the user dictionary.

If the input method is absent from the list in System Settings or cannot be selected after being added, signing out and back in may resolve the issue.

### iOS

iOS users can download **赣语输入法** from the App Store, except in the France storefront.

### Android

Download `GonnyuGeneralIME-version-android.apk` and open it on Android to install. On first launch, follow the in-app setup to enable the Gon(Gan) Chinese input method, then select it from the system input-method picker.

### Windows

Download and run `GonnyuGeneralIME-version-windows-installer.exe`. After the installer finishes, open **Settings → Time & language → Language & region → Chinese (Simplified) → Keyboards** and add **Gannyu**.

### Linux Fcitx5

Download `GonnyuGeneralIME-version-fcitx5.tar.gz`, extract it, and run the installer included in the archive:

```sh
tar -xzf GonnyuGeneralIME-version-fcitx5.tar.gz
cd GonnyuGeneralIME-version-fcitx5
./install.sh
```

Restart Fcitx5 with `fcitx5 -r`, or sign out and back in. Then add **Gannyu Gan / 赣语** in `fcitx5-configtool`.

### Rime

Download `GonnyuGeneralIME-version-rime-region.zip` for the required locality. The archive works with Rime front ends on every platform.

For Windows Weasel, copy the archive contents into `%APPDATA%\Rime` and redeploy from the input-method menu. For Linux Fcitx5 Rime, copy the contents into `~/.local/share/fcitx5/rime/`, redeploy, then select the locality from the schema menu. For iOS and Android, import or deploy the ZIP in the installed Rime front end.

### Input settings

All switches are enabled by default, with both newer and older readings selected. Changes take effect without reinstalling or redeploying.

| Setting | Effect |
| --- | --- |
| Annotations | Show or hide candidate readings and annotations. |
| Gan–Mandarin associations | Control associated Gan and Mandarin candidates and their annotations. Disabling this filters Mandarin words while retaining single-character candidates. |
| Synonym associations | Control associated synonym candidates and their annotations, independently of Gan–Mandarin associations and annotations. |
| Fuzzy matching | Master switch for Mandarin Pinyin compatibility, literary–colloquial matching, and other fuzzy rules. Mandarin Pinyin compatibility allows words to be found using Mandarin Pinyin. Literary–colloquial matching links literary and colloquial readings; disabling it preserves their labels. Other fuzzy rules control matching beyond compatibility for the special i and e phonemes. |
| Reading mode | Select both newer and older readings, newer readings, or older readings. This affects matching and the readings shown in annotations. |

Disabling fuzzy matching suspends its three child options while preserving their selections; these options appear greyed out on mobile.

| Platform | Access |
| --- | --- |
| macOS | Click **赣·locality** in the menu bar and open **输入设置**. |
| Windows | With the Gan input method active, open **输入设置** in the language bar. |
| iOS, Android | Open the Gan input method app and adjust **输入设置**. |
| Rime | Open the front end’s schema menu and select a switch or reading mode. |
| Native Linux Fcitx5 | These settings have no interface yet. Fcitx5 Rime uses the Rime schema menu. |

## The General Gon-pin Romanisation

The general Gon(Gan) romanisation (gon-pin) used by this project represents Gon(Gan) pronunciation while keeping its spellings as close as practical to Hanyu Pinyin for everyday input.

The system aims to cover phonemes across localities, represents necessary phonemic contrasts, and provides consistent notation for nasalised finals and voiced consonants in some Gon(Gan) varieties. To accommodate mergers in newer varieties and facilitate input, it does not attempt to transcribe every phonetic detail, such as breathy or slack phonation, and merges sound values that are not contrastive within a locality or occur as alternative pronunciations.

### Initials

| gon-pin | Default IPA | Fungcen IPA | Tiqien IPA | Sinyi IPA | Songau IPA | Seusong IPA | Jingon IPA | Yikyan-Henfeng IPA | Jisuibaedu IPA | Xyucuênn IPA | Singzï IPA | Goungon IPA | Accepted alternative input | Notes |
| --- | --- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --- | --- |
| ∅ | [] or [ʔ] | - | - | - | - | - | - | - | - | - | - | - |  | Zero initial |
| b | [p] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| p | [pʰ] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| m | [m] | - | - | - | - | - | - | - | - | - | - | - | - |  |
| f | [f] or [ɸ] | - | - | - | - | - | - | - | - | - | - | - | — | Usually transcribed as [f]; some consider it closer to [ɸ], a bilabial sound. |
| d | [t] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| t | [tʰ] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| l | [l] | - | - | - | - | - | - | - | - | - | - | - | - |  |
| z | [ts] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| c | [tsʰ] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| s | [s] | - | - | [θ] | - | - | - | - | - | - | - | - |  |  |
| j | [tɕ] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| q | [tɕʰ] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| n | [ȵ] | - | [n] | - | - | [n] or [ɳ] | - | [n] | [ɳ] | [n] | [n] | - | - |  |
| x | [ɕ] | - | - | － | － | - | - | - | - | - | - | - |  |  |
| g | [k] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| k | [kʰ] | - | - | - | - | - | - | - | - | - | - | - |  |  |
| ng | [ŋ] | - | - | - | - | - | - | - | - | - | - | - | - | Velar nasal; example: 五 ng3. |
| h | [h] | [x] | - | - | [x] | [x] | [x] | [x] | [x] | - | [x] | - | — | The default [h] is articulated farther back than Mandarin h. |
| v | - | [v] | [v] | - | [v] | - | - | - | - | - | - | - | w, wu (at the start of a syllable, one-way) |  |
| ch | - | - | - | - | - | [tʂʰ] | - | - | - | - | - | - |  |  |
| r | - | - | - | - | - | [ʐ] | - | - | - | - | - | - |  |  |
| sh | - | - | - | - | - | [ʂ] | - | - | - | - | [ʂ] | - |  |  |
| zh | - | - | - | - | - | [tʂ] | - | - | - | - | [tʂ] | - |  |  |
| bb | - | - | - | - | - | - | - | - | - | - | [b] | - | b (one-way) |  |
| dd | - | - | - | - | - | - | - | - | - | - | [d] | - | d (one-way) |  |
| gg | - | - | - | - | - | - | - | - | - | - | [ɡ] | - | g (one-way) |  |
| jj | - | - | - | - | - | - | - | - | - | - | [dʑ] | - | j (one-way) |  |
| zz | - | - | - | - | - | - | - | - | - | - | [dz] | - | z (one-way) |  |
| zzr | - | - | - | - | - | - | - | - | - | - | [dʐ] | - | zr, zh, zzh (one-way) |  |

### Finals

Checked-tone codas follow these spelling and input rules.

- The IPA coda [ʔ] is written as `-k` in Gon-pin.
- Checked-tone codas may be omitted during input while still matching the corresponding readings.
- Each locality independently specifies a combination of `-t`, `-p`, and `-k`, or no checked-tone codas. The codas enabled for a locality are accepted interchangeably within that locality.

#### Open finals

| gon-pin | Default IPA | Fungcen IPA | Tiqien IPA | Sinyi IPA | Songau IPA | Seusong IPA | Jingon IPA | Yikyan-Henfeng IPA | Jisuibaedu IPA | Xyucuênn IPA | Singzï IPA | Goungon IPA | Accepted alternative input | Notes |
| --- | --- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --- | --- |
| ae | - | [æ] | - | - | [æ] | [æ] | [æ] | - | [æ] | [æ] | - | - | — |  |
| a | [a] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| o | [o] | - | [ɔ] | - | - | - | [ɔ] | - | - | [ɔ] | [ɔ] | [ɔ] | — |  |
| oe | - | - | - | [ø] | - | - | [œ] or [ø] | - | - | - | - | - |  |  |
| e* | [e] | [ɛ] | [ɛ] | [ə] or [ɛ] | [ə] | - | [ə] or [ɛ] | [ə] or [ɛ] or [ɯ] | [ə] or [ɛ] | [ɛ] or [ɤ] | [ɛ] | [ə] or [ɛ] | ir (→ e), ee (→ ê) |  |
| eo | [ɵ] | - | - | - | - | - | - | - | - | - | - | - | o (standalone syllable, one-way) |  |
| ai | [ai] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| oi | [oi] | - | [ɔi] | [ɔi] | [ɔi] | - | [ɔi] | - | - | - | - | [ɔi] | — |  |
| ei | [ei] or [ɨi] | [ɛi] | [ɛi] | [əi] | - | - | - | - | - | - | - | [əi] or [ɛi] | — | － |
| eoi | - | - | - | [ɵi] | - | - | - | - | - | - | - | - | — |  |
| aeu | - | - | - | - | [æu] | - | - | - | - | - | - | - |  |  |
| au | [au] | [ɑu] | - | - | - | [ɑu] | - | - | - | - | - | - | ao |  |
| ou | - | - | [ɔu] | - | - | - | - | [ɔu] | - | - | - | [ɔu] | - |  |
| eu | [ɛu] or  [ɨu] | [əu] | - | [əu] or [ɪu] | - | [əu] | - | [əu] | - | - | [eu] | [əu] | ou (after some initials), ieu (after h) |  |
| am | - | [am] | [am] | - | - | - | - | - | - | - | - | - | — |  |
| om | - | - | [ɔm] | - | - | - | - | - | - | - | - | - | - |  |
| em | - | - | [ɛm] | - | - | - | - | - | - | - | - | - | - |  |
| aen | - | - | - | - | [æn] | - | - | - | - | - | - | - |  |  |
| an | [an] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| on | [on] | - | [ɔn] | [ɔn] | [ɔn] | - | [ɔn] | - | - | - | - | [ɔn] | — |  |
| oen | - | - | - | [øn] | - | - | - | - | - | - | - | - | — |  |
| en | [ɛn]  or  [ɨn] | [ən] | [ən] | [en] or [ɪn] | [ən] | [ən] | [ən] | [en] or [ɛen] or [ən] | - | - | [en] | [ən] | ing (Fungcen) |  |
| eon | - | - | - | - | - | - | - | - | - | - | - | [ɵn] |  |  |
| ann* | - | - | - | - | - | - | - | - | - | [ã] | - | - | an |  |
| ang | [ɑŋ] | - | [aŋ] | [aŋ] | - | - | - | - | - | - | - | - | — |  |
| ong | [ɔŋ] | [oŋ] | - | [oŋ] | - | [oŋ] | - | - | [oŋ] | - | - | - | on (Yikyan) | Yikyan does not distinguish front and back variants of ong. |
| eng | - | - | [ɛŋ] | - | - | - | [əŋ] | [əŋ] | - | [ɤŋ] | [eŋ] | - | en (Yikyan) | Most speakers no longer distinguish en and eng in Yikyan. |
| aet | - | [æt] | - | - | [æt] | - | - | - | - | - | - | - | — |  |
| at | [at] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| ot | [ot] | - | - | [ɔt] | [ɔt] | - | - | - | - | - | - | [ɔt] | — |  |
| oet | - | - | - | [øt] | - | - | - | - | - | - | - | - | — |  |
| et | [ɛt]  or  [ɨt] | - | - | [ət] or [ɪt] | - | - | - | - | - | - | - | [ət] | — |  |
| eot | - | [ɵt] | - | [ɵt] | - | - | - | - | - | - | - | - | — |  |
| aep | - | [æp] | - | - | - | - | - | - | - | - | - | - | — |  |
| ap | - | [ap] | [ap] | - | - | - | - | - | - | - | - | - | — |  |
| op | - | - | [ɔp] | - | - | - | - | - | - | - | - | - | — |  |
| ep | - | - | [ɛp] | - | - | - | - | - | - | - | - | - | - |  |
| eop | - | [ɵp] | - | - | - | - | - | - | - | - | - | - | — |  |
| aek | - | [æʔ] or [æk] | - | - | [æʔ] | - | - | - | - | - | - | - | — |  |
| ak | [aʔ] | - | - | - | - | - | - | - | - | - | - | [ak] | — |  |
| aik | - | - | [aiʔ] | [aiʔ] | - | - | - | - | - | - | - | - | - |  |
| ok | [ɔʔ] | [oʔ] | - | [oʔ] or [ɔk] | - | - | - | - | - | [oʔ] | - | [ɔk] | uok (Yikyan) |  |
| oek | - | - | - | [øʔ] | - | - | - | - | - | - | - | - | — |  |
| oik | - | - | [ɔiʔ] | - | - | - | - | - | - | - | - | - | - |  |
| ek | - | [ɛʔ] or [ɨʔ] | [ɛʔ] | [ɛʔ] or [əʔ] | - | - | - | [ɛʔ] or [ɪʔ] | - | [eʔ] | - | - | — |  |
| eok | - | [ɵʔ] | - | - | - | - | - | - | - | - | - | - | — |  |
| euk | - | - | - | [ɪuʔ] | - | - | - | - | - | - | - | - | — |  |

\* Nasalized finals are marked with `nn` at the end; a final single `n` can be typed as a one-way alias for `nn`, e.g. `an`→`ann`. The other nasalized finals in the tables follow the same rule. Omit `nn` when a coda follows.

\* Some regions use `ê` for IPA [ɛ] or [e]; typing `e` alone matches `ê`, `ir` precisely corresponds to Gon-pin `e`, and `ee` precisely corresponds to Gon-pin `ê`. The same applies to `ê` in other finals.

#### Front-vowel finals

With no initial consonant:

- Before `-a`, `-o`, or `-e`, initial `i` is written `y`.
- In other positions, it is written `yi`.

| gon-pin | Default IPA | Fungcen IPA | Tiqien IPA | Sinyi IPA | Songau IPA | Seusong IPA | Jingon IPA | Yikyan-Henfeng IPA | Jisuibaedu IPA | Xyucuênn IPA | Singzï IPA | Goungon IPA | Accepted alternative input | Notes |
| --- | --- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --- | --- |
| i* | [i]  or  [ɿ] | - | - | - | - | [ʅ] | - | - | - | - | [ʅ] | - | — |  |
| iae | - | - | - | - | [iæ] | - | - | - | - | [iæ] | - | - |  |  |
| ia | [ia] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| io | - | [iɔ] | [iɔ] | [io] | - | [io] | [io] or [iɔ] | [io] | [io] | [io] or [iɔ] | [io] or [iɔ] | [iɔ] | - |  |
| ie | [iɛ] | - | - | [ie] | - | [ie] | - | - | - | [ie] | - | - | — |  |
| iai | - | - | - | [iai] | - | - | - | - | - | - | - | - | - |  |
| ioi | - | - | - | [ioi] or [iɔi] | - | - | - | - | - | - | - | - | - |  |
| iu* | [iu] | - | - | - | - | - | - | - | - | - | - | - | iu, you (no initial) |  |
| iaeu | - | - | - | - | [iæu] | - | - | - | - | - | - | - |  |  |
| iau | - | [iau] | [iau] | [iau] | - | [iɑu] | [iau] | [iau] | [iau] | - | [iau] | [iau] | - |  |
| iou | - | - | - | - | - | - | [iɔu] | - | [iou] | - | - | - |  |  |
| ieu | [iɛu] | [iəu] | - | [iəu] | - | [iəu] | - | [iəu] | - | - | - | - | eu (after g, k, ng) |  |
| im | - | [im] | [im] | - | - | - | - | - | - | - | - | - | - |  |
| iam | - | - | [iam] | - | - | - | - | - | - | - | - | - | - |  |
| in | [in] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| iaen | - | - | - | - | [iæn] | - | - | - | - | - | - | - |  |  |
| ian | - | [ian] | - | [ian] | [ian] | - | - | [ian] | - | - | - | - | - |  |
| ion | - | - | - | [ion] or [iɔn] | [iɔn] | - | - | - | - | - | - | [iɔn] |  |  |
| ien | [iɛn] | - | - | [ien] | - | - | - | [ien] or [iən] | - | - | - | - | en (after g, k, ng) |  |
| iun | - | - | - | [iun] | - | - | - | - | - | - | - | - | - |  |
| inn | - | - | - | - | - | - | - | - | - | [ĩ] | - | - | in |  |
| iann | - | - | - | - | - | - | - | - | - | [iã] | - | - | ian |  |
| ienn | - | - | - | - | - | - | - | - | - | [iɛ̃] | - | - | ien |  |
| ing | - | - | [iŋ] | - | - | - | - | - | - | - | [iŋ] | - | - |  |
| iang | [iɑŋ] | - | [iaŋ] | [iaŋ] | - | - | - | - | - | [iãŋ] | - | - | - |  |
| iong | [iɔŋ] | [ioŋ] | - | [ioŋ] | - | [ioŋ] | - | - | [ioŋ] | - | - | - | - |  |
| iung | [iuŋ] | - | - | - | - | - | - | - | - | - | - | - | - |  |
| it | [it] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| iat | - | [iat] | - | [iat] | - | - | - | - | - | - | - | - | - |  |
| iot | - | - | - | [iɔt] | - | - | - | - | - | - | - | - | — |  |
| iet | [iet] | [iɛt] | - | [iɛt] or [iət] | [iɛt] | - | - | - | - | - | - | [iɛt] | et (after ng) |  |
| iut | - | - | - | [iut] | - | - | - | - | - | - | - | - | - |  |
| ip | - | [ip] | [ip] | - | - | - | - | - | - | - | - | - | - |  |
| iap | - | [iap] | [iap] | - | - | - | - | - | - | - | - | - | - |  |
| iep | - | [iɛp] | - | - | - | - | - | - | - | - | - | - | — |  |
| ik | [iʔ] | - | - | [ɿʔ] | - | - | - | - | - | - | - | - |  |  |
| iaek | - | - | - | - | [iæʔ] | - | - | - | - | - | - | - |  |  |
| iak | [iaʔ] | - | - | [iɑʔ] | - | - | - | - | - | - | - | [iak] | - |  |
| iaik | - | - | - | [iaiʔ] | - | - | - | - | - | - | - | - | - |  |
| iok | [iɔʔ] | - | - | [ioʔ] | - | - | - | - | - | [ioʔ] | - | [iɔk] | - |  |
| iek | - | [iɛʔ] | [iɛʔ] | [iɛʔ] or [iəʔ] | [iɛʔ] | - | - | [iɛʔ] or [iɪʔ] | - | [ieʔ] | - | - |  |  |
| iuk | [iuʔ] | - | - | - | - | - | - | - | - | - | - | [iuk] | - |  |
| iuok | - | - | - | [iuɔʔ] | - | - | - | - | - | - | - | - | - |  |
| iuek | - | - | - | [iuəʔ] | - | - | - | - | - | - | - | - | - |  |

\* `i`: Some regions use `ï` for IPA [ɿ], including [ɿ] in other finals. Input uses `i`, which can match `ï` through one-way fuzzy matching.

\* `iu`: Bare iu without an initial consonant is written yiu, distinct from yu for [y]. The same spelling applies when an entering-tone coda is attached directly to iu. Forms with other endings are an exception.

#### Rounded finals

With no initial consonant:

- Before `-a`, `-o`, `-e`, or `-i`, initial `u` is written `w`.
- In other positions, it is written `wu`.

| gon-pin | Default IPA | Fungcen IPA | Tiqien IPA | Sinyi IPA | Songau IPA | Seusong IPA | Jingon IPA | Yikyan-Henfeng IPA | Jisuibaedu IPA | Xyucuênn IPA | Singzï IPA | Goungon IPA | Accepted alternative input | Notes |
| --- | --- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --- | --- |
| u | [u] | - | - | - | - | [ʯ]* | - | - | - | - | - | - | - |  |
| uae | - | - | - | - | - | [uæ] or [ʯæ] | - | - | [uæ] | - | - | - |  |  |
| ua | [ua] | - | - | - | - | - | - | - | - | - | - | - | - |  |
| uo | [uo] | - | [uɔ] | - | - | - | - | - | - | - | - | [uɔ] | - |  |
| ue | [ue] | [uɛ] | [uɛ] | [uɛ] | - | - | [uɛ] | [uɛ] | [uɛ] | - | [uɛ] | - | - |  |
| uia | - | - | - | - | - | - | - | - | - | - | [uia] | - |  |  |
| uie | - | [uiɛ] | - | - | - | - | - | - | - | - | - | - | wie (no initial) |  |
| ui* | [ui] or [uei] | [uɛi] | - | [uəi] | - | [ʯei] | - | - | - | - | - | [uəi] or [uɛi] | uei, ui, wui (no initial), wei (no initial) |  |
| uai | [uai] | - | - | - | - | [ʯai] | - | - | - | - | - | - | - |  |
| uoi | - | - | [uɔi] | [uoi] or [uɔi] | - | - | - | [uoi] | - | - | - | - | — |  |
| ueu | - | - | - | [uəu] or [ueu] | - | - | - | - | - | - | - | - | - |  |
| un* | [un] or [uen] | [uɛn] | [uɛn] | - | - | [uən] or [ʯən] or [ʯɛn] | [uɛn] | [uɛn] | [uɛn] | - | - | [uən] or [uɛn] | uen |  |
| uan | [uan] | - | - | [uɑn] | - | - | - | - | - | - | - | - | - |  |
| uon | [uon] | - | [uɔn] | [uɔn] | - | - | [uɔn] | - | - | - | - | [uɔn] | uen, wen (no initial) |  |
| ueon | - | - | - | - | - | - | - | - | - | - | - | [uɵn] |  |  |
| uin | - | [uin] | - | [uin] | - | - | - | - | - | - | [uin] | - | - |  |
| uien | - | [uiɛn] | - | - | - | - | - | - | - | - | [uiɛn] | - | - |  |
| unn | - | - | - | - | - | - | - | - | - | [ũɛ̃] | - | - | un, uen, uenn |  |
| uann | - | - | - | - | - | - | - | - | - | [uã] | - | - | uan |  |
| ung | [uŋ] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| uang | [uɑŋ] | - | [uaŋ] | [uaŋ] | - | - | - | - | - | [uãŋ] | - | - | - |  |
| uong | [uɔŋ] | [uoŋ] | - | [uoŋ] | - | - | - | - | [uoŋ] | [uɔ̃ŋ] | - | - | uon (Yikyan) |  |
| ueng | - | - | - | - | - | - | - | [ueŋ] | - | - | [ueŋ] | - | — |  |
| ut | [ut] | - | - | - | - | - | - | - | - | - | - | - | - |  |
| uaet | - | [uæt] | - | - | - | - | - | - | - | - | - | - | - |  |
| uat | [uat] | - | - | - | - | - | - | - | - | - | - | - | - |  |
| uot | [uot] | - | - | [uɔt] | - | - | - | - | - | - | - | - | - |  |
| uet | [uɛt] | [uɨt] | - | [uət] | - | - | - | - | - | - | - | [uət] | - |  |
| uep | - | [uɛp] | - | - | - | - | - | - | - | - | - | - | - |  |
| uk | [uʔ] | - | - | - | - | - | - | - | - | - | - | [uk] | - |  |
| uaek | - | [uæʔ] | - | - | - | - | - | - | - | - | - | - | - |  |
| uak | [uaʔ] | - | - | - | - | - | - | - | - | - | - | - | - |  |
| uaik | - | - | [uaiʔ] | [uaiʔ] | - | - | - | - | - | - | - | - | - |  |
| uok | [uoʔ] | [uɔʔ] | [uɔʔ] | [uɔʔ] | - | - | - | [uɔʔ] | - | - | - | - | ok (Yikyan) |  |
| uoik | - | - | [uɔiʔ] | - | - | - | - | - | - | - | - | - | - |  |
| uek | — | [uɛʔ] or [uɨʔ] | [uɛʔ] | [uɛʔ] or [uəʔ] | - | - | - | [uəʔ] or [uɛʔ] or [uɪʔ] | - | [ueʔ] | - | - | — |  |
| uik | - | [uɛiʔ] | [uiʔ] | [uəiʔ] | - | - | - | - | - | - | - | - | ueik |  |

\* `u`: The rounded postalveolar apical vowel [ʯ] is written u; finals beginning with [ʯ] also use u-series spellings. The vowel and its medial forms correspond to the front rounded series and are listed under rounded finals by spelling.

\* `uei`: `uei` and `uêi` are shortened to `ui` after an initial consonant and grouped under `ui` in this table. With no initial, the forms are `wei` and `wêi`; [ui] has the form `wi`. The same applies to checked-tone forms.

\* `uen` and `uên` shorten to `un` after an initial and are grouped under `un`; zero-initial forms are `wen` and `wên`. Nasalized forms follow the same pattern.


#### Rounded front-vowel finals

`yu` is provisionally used throughout for [y].

| gon-pin | Default IPA | Fungcen IPA | Tiqien IPA | Sinyi IPA | Songau IPA | Seusong IPA | Jingon IPA | Yikyan-Henfeng IPA | Jisuibaedu IPA | Xyucuênn IPA | Singzï IPA | Goungon IPA | Accepted alternative input | Notes |
| --- | --- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --- | --- |
| yu | [y] | - | - | - | - | - | - | - | - | - | - | - | v (after an initial only), u (after an initial only, never immediately after y) |  |
| yua | - | - | - | - | - | - | - | [ya] | - | - | - | - | — |  |
| yue | [ye] | - | - | - | - | - | [yɛ] | [yɛ] | [yə] | - | - | - | - |  |
| yueo | - | [yɵ] | - | - | - | - | - | - | - | - | - | - | — |  |
| yuoi | - | - | - | - | - | - | - | [yoi] | - | - | - | - | — |  |
| yun | [yn] | - | - | - | - | - | - | - | - | - | - | - | — |  |
| yuon | [yon] | - | - | [yɔn] | - | - | [yɔn] | - | - | - | - | [yɔn] | yoin (without an initial), yuen |  |
| yuen | - | - | - | - | - | - | [yɛn] | [yɛn] or [yɛŋ] | - | - | - | - | yueng |  |
| yuenn | - | - | - | - | - | - | - | - | - | [yɛ̃] | - | - | yuen |  |
| yuinn | - | - | - | - | - | - | - | - | - | [yĩ] | - | - | yuin |  |
| yung | [yŋ] | - | - | - | - | - | - | - | - | - | - | - | yun (Yikyan) |  |
| yuong | - | - | - | - | - | - | - | [yɔŋ] | - | - | - | - |  |  |
| yueng | - | - | - | - | - | - | - | [yeŋ] | - | - | - | - | — |  |
| yut | [yt] | - | - | - | - | - | - | - | - | - | - | - | - |  |
| yuot | [yot] | - | - | [yɔt] | - | - | - | - | - | - | - | [yɔt] | yue, yuet, yuek |  |
| yuet | - | [yet] | - | [yət] | - | - | - | - | - | - | - | - | - |  |
| yuep | - | [yep] | - | - | - | - | - | - | - | - | - | - | — |  |
| yuk | - | [yʔ] | [yʔ] | [yʔ] | - | - | - | [yʔ] | - | - | - | - | - |  |
| yuak | - | - | - | - | - | - | - | [yaʔ] | - | - | - | - | - |  |
| yuok | - | - | - | [yɔʔ] | - | - | - | [yɔʔ] | - | - | - | - | - |  |
| yuek | - | [yeʔ] | - | [yəʔ] | - | - | - | [yəʔ] or [yɛʔ] or [yɪʔ] | - | [yeʔ] | - | - | — |  |
| yueok | - | [yɵʔ] | - | - | - | - | - | - | - | - | - | - | — |  |

#### Syllabic laterals and nasals

| gon-pin | Default IPA | Fungcen IPA | Tiqien IPA | Sinyi IPA | Songau IPA | Seusong IPA | Jingon IPA | Yikyan-Henfeng IPA | Jisuibaedu IPA | Xyucuênn IPA | Singzï IPA | Goungon IPA | Notes |
| --- | --- | --- | --- | --- | --- | --- | ------- | ------- | ------- | ------- | ------- | ------- | --- |
| m | [m̩] | - | - | - | - | - | - | - | - | - | - | - |  |
| n | [n̩] | - | - | - | - | - | - | - | - | - | - | - |  |
| ng | [ŋ̩] | - | [ŋ̍] | [ŋ̍] | [ŋ̍] | - | [ŋ̍] | [ŋ̍] | [ŋ̍] | - | - | [ŋ̍] |  |
| l | - | - | - | - | - | [ɭ̩] | - | - | - | - | - | - |  |

#### Other segments

The system also records a number of extensions based on published descriptions and observed sound changes, including pronunciations recorded in a 1935 language survey and selected alternations involving `-n` and `-ng` codas.

### Tones

`0` denotes the neutral tone.

#### Lancong (Nanchang)

The Lancong (Nanchang) dictionary uses seven tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 42 |
| 2 | yang level | 24 |
| 3 | rising | 213 |
| 4 | yin departing | 44(5) |
| 5 | yang departing | 21 |
| 6 | yin checked | 5 |
| 7 | yang checked | 1 or 2 |

#### Fungcen (Fengcheng)

The Fungcen (Fengcheng) dictionary uses six tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 33 |
| 2 | yang level | 35 |
| 3 | rising | 213 |
| 4 | departing | 31 |
| 5 | yin checked | 1 |
| 6 | yang checked | 5 |

#### Tiqien (Lichuan)

The Tiqien (Lichuan) dictionary uses the following tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 22 |
| 2 | yang level | 35 |
| 3 | rising | 44 |
| 4 | yin departing | 53 |
| 5 | yang departing | 13 |
| 6 | yin checked | 3 |
| 7 | yang checked | 5 |

#### Sinyi (Xinyu City)

The Sinyi (Xinyu City) dictionary uses the following tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level A | 45 |
| 1* | yin level B | 24 |
| 2 | yang level | 31 |
| 3 | rising | 213 |
| 4 | departing A | 11 |
| 4* | departing B | 33 |
| 5 | yin checked A | 5 |
| 5* | yin checked B | 24 |
| 6 | yang checked | 3 |

Departing B and yang checked occur around Shuibei; in the urban area, they have merged with yang level and yin checked B, respectively.

#### Songau (Shanggao)

The Songau (Shanggao) dictionary uses the following tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 31 |
| 2 | yang level | 24 |
| 3 | rising | 213 |
| 4 | departing | 51 |
| 5 | checked | 4 |

#### Seusong (Susong)

The Seusong (Susong) dictionary uses the following tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 22 |
| 2 | yang level | 35 |
| 3 | rising | 51 |
| 4 | yin departing | 21 |
| 5 | yang departing | 314 |
| 6 | checked | 55 |

#### Jingon (Jian City)

The Jingon (Jian City) dictionary uses the following tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 334 |
| 2 | yang level | 11 |
| 3 | rising | 53 |
| 4 | departing | 214 |
| 5 | high rising | 35 |

#### Yikyan-Henfeng (Yiyang-Hengfeng)

The Yikyan-Henfeng (Yiyang-Hengfeng) dictionary uses the following tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 44 |
| 2 | yang level | 24 |
| 3 | rising | 42 |
| 4 | yin departing | 35 |
| 5 | yang departing | 21 |
| 6 | yin checked | 5 |
| 7 | yang checked | 4 |

#### Jisuibaedu (Jishuibadou)

The Jisuibaedu (Jishuibadou) dictionary uses the following tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 44 |
| 2 | yang level | 53 |
| 3 | rising | 213 |
| 4 | departing | 31 |

#### Xyucuênn (Suichuan)

The Xyucuênn (Suichuan) dictionary uses the following tone markers:

| Marker | Tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 52 |
| 2 | yang level | 22 |
| 3 | yin rising | 21 |
| 4 | yang rising | 25 |
| 5 | departing A | 215 |
| 5* | departing B | 55 |
| 6 | checked | 5 |

#### Singzï (Xingzi)

The Singzï (Xingzi) dictionary uses the following tone markers:

| Marker | Traditional tone category | Pitch value |
| --- | --- | --- |
| 1 | yin level | 33 |
| 2 | yang level | 213 |
| 3 | rising | 42 |
| 4 | yin departing A | 34 |
| 4* | yin departing B | 324 |
| 5 | yang departing | 21 |
| 6 | checked | 35 |

#### Goungon (Goungon)

Goungon tones are shown under the same tone markers with separate columns for literature and source-data notation and the project's interpretation. The source-data column retains the dictionary source's pitch values. The project's interpretation was proposed by the Goungon dictionary contributor from auditory observations, experimental phonetic measurements, and comparison with the literature.

| Marker | Traditional tone category | Literature and source-data pitch value | Project interpretation |
| --- | --- | --- | --- |
| 1 | yin level | 35 | 25 |
| 2 | yang level | 13 | 213 |
| 3 | rising | 42 | 42 |
| 4 | yin departing | 33 | 33 |
| 5 | yang departing | 11 | 11 |
| 6 | yin checked | 5 | 3 |
| 7 | yang checked | 1 | 1 |

The Goungon dictionary contributor supplied the following two sets of measurements. The table gives measured values. In the first set, yang level and rising were originally notated as 213 and 42; in the second set, yang level was originally notated as 13.

| Measurement | Yin level | Yang level | Rising | Yin departing | Yang departing | Yin checked | Yang checked |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Set 1 | 25 | 112 | 41 | 33 | 11 | 3 | 1 |
| Set 2 | 25 | 12 | 41 | 33 | 11 | 3 | 1 |

The contributor also reports alternative literature notations of 213 for yang level, 44 or 55 for yin departing, 22 or 33 for yang departing, 4 or 3 for yin checked, and 2 or 3 for yang checked. Repeated measurements gave 12 or 112 for yang level, with the rise remaining below level 3; 41 or 42 for the rising tone; and usually 3, approaching level 4, for yin checked. Yang checked was also measured as 21 with a falling contour.

## References

The project compiles its regional dictionaries from participants’ everyday language observations, academic publications, and dialect-enthusiast communities. Dictionary data and reference materials are made public as far as possible. Copyright enquiries can be sent to the contacts listed at the end of this document.

### Literature

1. osfans. **MCPDict** [CP/OL]. GitHub. <https://github.com/osfans/MCPDict>.
2. Xiong Zhenghui. *Literary and colloquial readings in the Lancong (Nanchang) dialect* [EB/OL]. <http://ling.cass.cn/keyan/xueshuchengguo/cgtj/202112/W020211223381176680381.pdf>. Accessed 2026-06-01.
3. Xiong Zhenghui. *Difficult characters in the Lancong (Nanchang) dialect* [EB/OL]. <http://ling.cass.cn/keyan/xueshuchengguo/cgtj/202112/W020211223381177519680.pdf>. Accessed 2026-06-04.
4. Xiong Zhenghui. *Dictionary of the Lancong (Nanchang) Dialect*.
5. Zhihu. “What vocabulary is distinctive enough to identify Gon(Gan) Chinese at once?” <https://www.zhihu.com/question/24262923/>.
6. Wikipedia. *Gon(Gan) Chinese original characters*. <https://gan.wikipedia.org/wiki/>.
7. Wikipedia. *Gon(Gan) Chinese*. <https://zh.wikipedia.org/zh-hans/%E8%B4%9B%E8%AA%9E>.
8. *Character-use standards for Chinese dialects, Language Resources Protection Project of China*. <http://www.moe.gov.cn/s78/A19/tongzhi/201704/W020170405307025943395.pdf>. Accessed 2026-08-04.
9. Bilibili. *New Concept Lancong (Nanchang) Dialect* series. <https://www.bilibili.com/video/BV1Us4y1C7fp/?share_source=copy_web&vd_source=5078721afbb2afc4394ca2602bb990de>.
10. Xiao Ping and Xiao Jiehan. *Dictionary of the Wucheng Dialect of Jiangxi* [M]. Beijing: The Commercial Press, 2017. Bibliographic information is listed in the [linguistic bibliography](https://geolinguistics.sakura.ne.jp/Monograph/SIG-Mono7-LAAA-3-ebook.pdf).
11. Li Rong, general editor; Yan Sen, compiler. *Dictionary of the Lichuan Dialect* (黎川方言词典) [M]. Nanjing: Jiangsu Education Press, December 1995. ISBN 7-5343-2626-5. [Catalogue](https://cir.nii.ac.jp/crid/1971993809687512866).
12. Yang Shifeng. *Nanchang Phonology* (南昌音系) [J]. Bulletin of the Institute of History and Philology, 1969, 39(1): 125–204. Includes material from the 1935 survey. [Full text](https://www11.ihp.sinica.edu.tw/storage/w2_file/3942MsWGVpd.pdf).
13. Li Rulong and Zhang Shuangqing, editors. *Survey Report on Hakka and Gan Dialects* (客赣方言调查报告) [M]. Xiamen: Xiamen University Press, 1992. ISBN 7-5615-0385-7. [Catalogue](https://books.google.com/books?id=ApAtAQAAIAAJ).
14. Jiangxi Provincial Local Gazetteer Compilation Committee; Chen Changyi, editor. *Gazetteer of the Dialects of Jiangxi Province* (江西省方言志) [M]. Beijing: Fangzhi Press, 2005. *Jiangxi Provincial Gazetteer*, vol. 96. ISBN 7-80192-490-8. [Catalogue](https://search.worldcat.org/title/70105497).
15. Xiao Jiugen. *A Study of the Badu Dialect* (八都方言研究) [D]. Master's thesis, Guangxi University, 2001.
16. Mei Zhixing. *A Study of Variation in the Central Rounded Vowel [ɵ] in the Goungon Dialect* (高安方言央圆唇元音[ɵ]的变异研究) [Dissertation]. Guangdong University of Foreign Studies, 2020.

### Reference implementations

Mobile keyboard interactions reference the public implementations of [Hamster (open-source version)](https://github.com/imfuxiao/Hamster), [Trime](https://github.com/osfans/trime), and [HeliBoard](https://github.com/HeliBorg/HeliBoard).

The open-source version of Hamster is by xiao.fu (imfuxiao), licensed under the [MIT License](https://github.com/imfuxiao/Hamster/blob/main/LICENSE.txt). Copyright (c) 2025 xiao.fu.

### Dependency declarations

The mobile input engine directly uses the following open-source projects. Exact revisions are pinned by the build lock; distributions retain the corresponding licences and copyright notices.

1. RIME Developers. **librime** `1.17.0`, BSD 3-Clause License. <https://github.com/rime/librime>
2. librime-lua Developers. **librime-lua** commit `ad1e4a6c98abf634dd34242a747f9b1d5d069fbe`, BSD 3-Clause License. <https://github.com/hchunhui/librime-lua>

### Licensing and rights reservation

The main project code and resource is licensed under the GNU GPLv3; see `LICENSE` for details.
The name and branding “赣语通用输入法” (abbreviated as “赣语输入法”), together with the image at
`resources/icon.png`, are not covered by the GPLv3. All related copyrights, trademark rights,
and other rights are reserved by their respective rights holder. Use of the name or image in
derivative projects, redistributions, or commercial promotion requires permission.

Copyright in the project documentation belongs to the rights holder, with all rights reserved. Verbatim republication is permitted with attribution to the author and project.

### Acknowledgements

Special thanks to @豫章鸿也 for extensive advice on the project’s romanisation and character and word choices.

Thanks also to **岭北南安**, whose work on the geographical distribution of Jiangxi vocabulary provided important inspiration for this input method.

Given the scale of the dictionaries and the author's limited expertise and time, errors may remain. The author takes responsibility for them. Contributions of additions and corrections are appreciated.

## Origins: the loss of Gon(Gan)

> After three months of work, I wanted to add an essay introducing the project.

The Gon(Gan)–Poyang plain has long been an important economic and cultural hub in southern China, a densely populated land of rice and fish. Since the late Qing period, Jiangxi and neighbouring areas have experienced severe economic and demographic decline for various reasons. When the economic foundations of a flourishing culture collapse, its standing declines as well. Among southern Sinitic languages, Gon(Gan) now seems to have the weakest cultural presence. Gon(Gan)-speaking areas lack the clearly recognised standard pronunciation associated with Cantonese, the economic resources of Wu-speaking areas, the familiar cultural symbols and overseas influence of Southern Min, or the firm social foundations of Southwestern Mandarin and Sichuan speech in the densely settled Sichuan Basin. People who speak Gon(Gan) or grew up in Gon(Gan)-speaking areas often lack a clear understanding of their language. Their understanding tends to be a vague one shaped by administrative boundaries: Jiangxi speech, or one of the local varieties of Hunan, Hubei, Anhui, and so on. The familiar notion that pronunciation changes every ten li encourages a view of the region as a collection of disconnected local dialects.

Many local cities are now trying to develop cultural tourism, but culture needs distinctive features to sustain it. In common perceptions of its culture and social life, the Gon(Gan)–Poyang plain has become one of the least distinctive inland Han Chinese regions. The Jiangxi merchant networks and the saying “half the court's civil and military officials came from Jiangxi” are relics of the past. Jiangxi once had a flourishing scholarly tradition. The mindset of the imperial examinations remains deeply rooted, but there are too few strong universities and too little to attract talent from elsewhere. The former pursuit of culture now finds expression in fierce competition over the national college entrance examination. Successful students leave in large numbers instead of staying in hometowns they regard as ordinary and provincial.

At the same time, residents who see their region as lacking distinctive features have been particularly quick to abandon those they do have. Around the time I was born, Gon(Gan)-speaking areas were already abandoning their home language on a large scale, despite its history stretching back thousands of years, and treating it as rustic, backward, and nonstandard. In primary school, I showed my writing to classmates; when it came back, every instance of `好 X` had been changed, with good intentions, to `很 X`. Yet some people like me, who were taught Mandarin first, did not become proficient Mandarin speakers. Gon(Gan) left deep traces in our vowels and consonants, everyday speech, and vocabulary in school compositions. Our command could even be weaker than that of the previous generation's bilingual speakers, who had studied Mandarin systematically. In school essays, for example, I repeatedly used `紧` to mean “always”, actually corresponding to the `尽` in `尽管`, and frequently used `嘎` as a sentence-initial element. My Mandarin was, in effect, a “creole within the Sinitic family”. Some of my classmates from Nanchang, whose social lives were tightly controlled by parents focused on academic success, could no longer understand Nanchang Gon(Gan) at all, or even distinguish it from neighbouring languages. This degree of loss may be relatively rare among Sinitic-speaking communities.

Reports that most of the world's languages could disappear before the end of this century may leave most Sinitic-speaking communities unmoved. They may not realise that, if current trends continue, **their own languages**, the local Chinese spoken at home or in their hometowns, could also join the list of disappearing languages. Older patterns of social life and local social memory are being forgotten along with them. Later generations looking back at those communities may unconsciously assume that life in their streets and villages was conducted in Mandarin. Local ways of using language will be forgotten. For Sinitic-speaking communities, this would be a severe and profound cultural loss.

We believe our work is an important part of protecting Sinitic linguistic diversity.

## Contributors and contact

1. Dongche Xiye Editorial Department. Project planning and the Fenni (Fenyi) dictionary. <https://github.com/ComeRainOrComeShine>
2. Doohaey. Input-method framework and the Lancong (Nanchang) dictionary. Email: doohaey@gmail.com
3. Hialex. App icon design.
4. AstroChung. Yikyan (Yiyang) dictionary maintenance.
5. 江南西道客. Fungcen (Fengcheng) dictionary resources. Email: yunmoqingchen@qq.com.
6. 剑邑 Jason. Fungcen (Fengcheng) testing and feedback.
7. Brian Z. Xinyu (新喻) readings and dictionary entries. <https://github.com/BrianIZKom1911>
8. 阿星. Goungon phonology and dictionary resources.
