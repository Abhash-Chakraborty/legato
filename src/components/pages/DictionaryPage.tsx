import React from "react";
import { useTranslation } from "react-i18next";
import { Page } from "../shell/Page";
import { SettingsGroup } from "../ui/SettingsGroup";
import { CustomWords } from "../settings/CustomWords";
import { WordCorrectionThreshold } from "../settings/debug/WordCorrectionThreshold";

/** The words Legato should spell your way, and how closely a heard word has
 *  to match one before it is swapped in. */
export const DictionaryPage: React.FC = () => {
  const { t } = useTranslation();
  return (
    <Page title={t("dictionary.title")} subtitle={t("dictionary.subtitle")}>
      <SettingsGroup title={t("dictionary.groups.words")}>
        <CustomWords grouped />
      </SettingsGroup>
      <SettingsGroup title={t("dictionary.groups.matching")}>
        <WordCorrectionThreshold grouped />
      </SettingsGroup>
    </Page>
  );
};
