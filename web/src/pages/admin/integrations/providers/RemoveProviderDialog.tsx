import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { call, client } from '../../../../api/client';
import { errorMessage } from '../../../../api/messages';
import { ConfirmDialog } from '../../../../components/ConfirmDialog';
import { useToast } from '../../../../components/Toast';
import { t } from '../../../../i18n/messages';
import { ti } from '../../../../i18n/messages.integrations';
import { cx } from '../../../../styles/cx';
import shared from '../../../../styles/shared.module.css';
import type { Provider } from '../api';
import { DEFAULTS_KEY, PROVIDERS_KEY } from '../api';

function defaultName(which: string): string {
  return which === 'orchestrator' ? ti('defaultsOrchestrator') : ti('defaultsAgents');
}

interface RemoveProviderDialogProps {
  provider: Provider | null;
  onClose: () => void;
}

/** Confirms removing a provider, naming the defaults it clears (BR-15.3). */
export function RemoveProviderDialog({ provider, onClose }: RemoveProviderDialogProps) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [failure, setFailure] = useState<string | null>(null);
  const id = provider?.id ?? '';

  const impact = useQuery({
    queryKey: ['admin', 'providers', id, 'removal-impact'],
    queryFn: () =>
      call(client.GET('/api/admin/providers/{id}/removal-impact', { params: { path: { id } } })),
    enabled: provider !== null,
    staleTime: 0,
    gcTime: 0,
  });

  const remove = useMutation({
    mutationFn: () =>
      call(client.DELETE('/api/admin/providers/{id}', { params: { path: { id } } })),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: PROVIDERS_KEY });
      void queryClient.invalidateQueries({ queryKey: DEFAULTS_KEY });
      toast.show('success', ti('providerRemoved'));
      setFailure(null);
      onClose();
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  const models =
    provider?.models.length === 1
      ? ti('modelCountOne')
      : ti('modelCount', { count: provider?.models.length ?? 0 });
  const cleared = impact.data?.cleared_defaults ?? [];

  return (
    <ConfirmDialog
      open={provider !== null}
      title={ti('removeTitle', { name: provider?.name ?? '' })}
      confirmLabel={ti('removeConfirm')}
      destructive
      busy={remove.isPending || impact.isPending || impact.isError}
      testId="remove-provider"
      onConfirm={() => {
        remove.mutate();
      }}
      onCancel={() => {
        setFailure(null);
        onClose();
      }}
    >
      <p>{ti('removeBody', { models })}</p>
      {impact.isPending ? (
        <p role="status">{t('loading')}</p>
      ) : impact.isError ? (
        <p className={cx(shared.banner, shared.bannerError)} role="alert">
          {ti('removeImpactError')}
        </p>
      ) : (
        <p data-testid="remove-provider-impact">
          {cleared.length === 0
            ? ti('removeNoDefaults')
            : ti('removeClears', { defaults: cleared.map(defaultName).join(', ') })}
        </p>
      )}
      <p>{ti('removeSessions')}</p>
      {failure === null ? null : (
        <p role="alert" className={cx(shared.banner, shared.bannerError)}>
          {failure}
        </p>
      )}
    </ConfirmDialog>
  );
}
