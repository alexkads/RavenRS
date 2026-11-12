# ravenrs-app-nfe

> **BETA, proof of concept.** Not homologated with any SEFAZ, and it cannot issue in production: the only environments are the local sandbox and homologation, which have no fiscal value. Everyone using an application with this module sees a notice that cannot be dismissed.

A module for `ravenrs-app` that issues NF-e and NFC-e through `ravenrs-nfe`, and the NFS-e through `ravenrs-nfse`: the issuer's record, the documents issued, consulting and canceling, and the receipt.

## The receipt is the customer's

The receipt is a stored report on the entity `NfeCupomLinha`, with one row for each item and everything else repeated on each row. Models for the 80 mm and 58 mm rolls come with the module, and anyone changes them in the standard report designer, which prints straight to ESC/POS printers. Printing checks only what makes it a fiscal receipt: the key, the QR Code, the number and the protocol have to be in the layout, and the "no fiscal value" notice comes back by itself if someone takes it out.

## The NFS-e

The same service issues the NFS-e of the national standard through `ravenrs-nfse`, to its own sandbox or, in homologation, to the restricted production of the Sefin Nacional. `emitir_nfse` gives the DPS its number and keeps it as a `NfseDocumento` before anything goes. A DPS that does not get a sure answer, or that was on its way when the program closed, stays pending; `reenviar_nfse` sends the same one again, with the same number, and the Sefin either finds the NFS-e it already made or makes it now. The DANFSe A4 is a stored report on `NfseLinha`, like the receipt.

## Limits

Simples Nacional only (CRT 1), synchronous authorization only, no DANFE for model 55, no void of number ranges. What each rule of the document came from, and what has no source yet, is in `crates/ravenrs-nfe/REGRAS.md`. What is missing for a product is in `docs/AUTOMACAO-COMERCIAL.md`.

Licensed under MIT or Apache-2.0.
