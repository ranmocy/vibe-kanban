import { useEffect, useRef, useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/lib/modals';

export interface MergeDialogProps {
  defaultMessage: string;
}

export type MergeDialogResult = {
  action: 'confirmed' | 'canceled';
  commitMessage?: string;
};

const MergeDialogImpl = NiceModal.create<MergeDialogProps>(
  ({ defaultMessage }) => {
    const modal = useModal();
    const [commitMessage, setCommitMessage] = useState(defaultMessage);
    const textareaRef = useRef<HTMLTextAreaElement>(null);

    useEffect(() => {
      setCommitMessage(defaultMessage);
    }, [defaultMessage]);

    useEffect(() => {
      if (modal.visible && textareaRef.current) {
        textareaRef.current.focus();
        textareaRef.current.select();
      }
    }, [modal.visible]);

    const handleConfirm = () => {
      modal.resolve({
        action: 'confirmed',
        commitMessage: commitMessage.trim(),
      } as MergeDialogResult);
      modal.hide();
    };

    const handleCancel = () => {
      modal.resolve({ action: 'canceled' } as MergeDialogResult);
      modal.hide();
    };

    const handleOpenChange = (open: boolean) => {
      if (!open) {
        handleCancel();
      }
    };

    return (
      <Dialog open={modal.visible} onOpenChange={handleOpenChange}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Merge Branch</DialogTitle>
            <DialogDescription>
              Are you sure you want to merge this branch into the target branch?
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-2">
            <label htmlFor="commit-message" className="text-sm font-medium">
              Commit message
            </label>
            <Textarea
              ref={textareaRef}
              id="commit-message"
              value={commitMessage}
              onChange={(e) => setCommitMessage(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                  handleConfirm();
                }
              }}
              rows={3}
            />
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={handleCancel}>
              Cancel
            </Button>
            <Button onClick={handleConfirm}>Merge</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const MergeDialog = defineModal<MergeDialogProps, MergeDialogResult>(
  MergeDialogImpl
);
